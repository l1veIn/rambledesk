import { get } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { defineAttachmentCandidate } from '../capabilities/capturePlugin'
import { createAttachmentController } from './attachmentController'
import {
  controllerContext,
  fileCandidate,
  mocks,
  resetAttachmentMocks,
} from './attachmentControllerTestHarness'

describe('attachmentController attachment writes', () => {
  beforeEach(() => {
    resetAttachmentMocks()
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('imports multiple server attachment paths with a continuing CAS revision', async () => {
    const workspace = {
      request: { request_id: 'request-1' },
      attachments: [],
      draft: { saved_revision: 3 },
    }
    const inserted = {
      ...workspace,
      draft: { saved_revision: 4 },
      attachments: [
        { attachment_id: 'att-1', file_name: 'notes.txt', media_type: 'text/plain' },
      ],
    }
    const insertedAgain = {
      ...inserted,
      draft: { saved_revision: 5 },
      attachments: [
        ...inserted.attachments,
        { attachment_id: 'att-2', file_name: 'other.txt', media_type: 'text/plain' },
      ],
    }
    const { context, session } = controllerContext()
    context.getWorkspace = () => workspace as never
    mocks.applicationCall.mockImplementation(async (operation: string) => {
      if (operation === 'getFeedbackWorkspace') return workspace
      return undefined
    })
    mocks.importAttachmentPath
      .mockResolvedValueOnce(inserted)
      .mockResolvedValueOnce(insertedAgain)

    await createAttachmentController(context).importServerAttachmentPaths([
      '/tmp/notes.txt',
      '/tmp/other.txt',
    ])

    expect(context.saveDraftNow).toHaveBeenCalled()
    expect(mocks.importAttachmentPath).toHaveBeenCalledWith({
      requestId: 'request-1',
      path: '/tmp/notes.txt',
      expectedRevision: 3,
    })
    expect(mocks.importAttachmentPath).toHaveBeenCalledWith({
      requestId: 'request-1',
      path: '/tmp/other.txt',
      expectedRevision: 4,
    })
    expect(context.applyWorkspaceMutation).toHaveBeenLastCalledWith(insertedAgain)
  })


  it('routes file-picker attachments to the request and Action captured at selection time', async () => {
    const target = {
      request: { request_id: 'request-1' },
      attachments: [],
      draft: { saved_revision: 1 },
    }
    const other = {
      request: { request_id: 'request-2' },
      attachments: [],
      draft: { saved_revision: 8 },
    }
    const inserted = {
      ...target,
      attachments: [
        {
          attachment_id: 'att-1',
          file_name: 'notes.txt',
          media_type: 'text/plain',
        },
      ],
      draft: { saved_revision: 2 },
    }
    let visible = target
    const { context, session } = controllerContext()
    context.getWorkspace = () => visible as never
    context.activeActionFor = (() => ({
      actionId: 'action-a',
      actionIndex: 0,
      title: 'First',
    })) as never
    context.saveDraftNow = vi.fn(async () => {
      visible = other
      return true
    })
    mocks.applicationCall.mockImplementation(async (command: string) => {
      if (command === 'getFeedbackWorkspace') return target
      if (command === 'addFeedbackAttachment') return inserted
      return undefined
    })
    const candidate = fileCandidate({
      fileName: 'notes.txt',
      byteLength: 4,
      readBytes: async () => new Uint8Array([1, 2, 3, 4]).buffer,
    })

    await createAttachmentController(context).importAttachmentCandidates([candidate])

    expect(context.routeDraftOperation).toHaveBeenCalledWith(
      'request-1',
      expect.objectContaining({
        kind: 'appendAttachment',
        attachment: inserted.attachments[0],
        action: { actionId: 'action-a', actionIndex: 0, title: 'First' },
      }),
    )
    expect(context.applyWorkspaceMutation).not.toHaveBeenCalled()
  })


  it('serializes candidate batches through one queue and carries the latest CAS revision forward', async () => {
    let current = {
      request: { request_id: 'request-1', status: 'in_progress' },
      attachments: [] as Array<{
        attachment_id: string
        file_name: string
        media_type: string
      }>,
      draft: { saved_revision: 3 },
    }
    const { context, session } = controllerContext()
    context.getWorkspace = () => current as never
    context.applyWorkspaceMutation = vi.fn((next) => {
      current = next as never
    })
    mocks.applicationCall.mockImplementation(async (command: string, input: {
      file_name?: string
      expected_revision?: number
    }) => {
      if (command === 'getFeedbackWorkspace') return current
      if (command === 'addFeedbackAttachment') {
        const nextRevision = (input.expected_revision ?? 0) + 1
        return {
          ...current,
          attachments: [
            ...current.attachments,
            {
              attachment_id: `att-${nextRevision}`,
              file_name: input.file_name ?? 'attachment.bin',
              media_type: 'application/octet-stream',
            },
          ],
          draft: { saved_revision: nextRevision },
        }
      }
      return undefined
    })
    const firstDispose = vi.fn(async () => undefined)
    const secondDispose = vi.fn(async () => undefined)
    const first = defineAttachmentCandidate({
      id: 'candidate-1',
      source: 'screen-capture',
      fileName: 'first.txt',
      mediaType: 'text/plain',
      byteLength: 1,
      readBytes: async () => new Uint8Array([1]).buffer,
      dispose: firstDispose,
    })
    const second = defineAttachmentCandidate({
      id: 'candidate-2',
      source: 'clipboard-image',
      fileName: 'second.txt',
      mediaType: 'text/plain',
      byteLength: 1,
      readBytes: async () => new Uint8Array([2]).buffer,
      dispose: secondDispose,
    })
    const controller = createAttachmentController(context)
    const target = { requestId: 'request-1', action: null }

    await Promise.all([
      controller.persistAttachmentCandidates(target, [first]),
      controller.persistAttachmentCandidates(target, [second]),
    ])

    const attachmentCalls = mocks.applicationCall.mock.calls.filter(
      ([command]) => command === 'addFeedbackAttachment',
    )
    expect(attachmentCalls.map(([, input]) => input.expected_revision)).toEqual([3, 4])
    expect(context.recordAttachmentDiagnostic).toHaveBeenNthCalledWith(
      1,
      'screen_capture_imported',
      'request-1',
    )
    expect(context.recordAttachmentDiagnostic).toHaveBeenNthCalledWith(
      2,
      'clipboard_image_imported',
      'request-1',
    )
    expect(firstDispose).toHaveBeenCalledOnce()
    expect(secondDispose).toHaveBeenCalledOnce()
  })


  it('disposes every candidate exactly once when candidate persistence fails', async () => {
    const workspace = {
      request: { request_id: 'request-1', status: 'in_progress' },
      attachments: [],
      draft: { saved_revision: 3 },
    }
    const { context, session } = controllerContext()
    context.getWorkspace = () => workspace as never
    mocks.applicationCall.mockImplementation(async (command: string) => {
      if (command === 'getFeedbackWorkspace') return workspace
      if (command === 'addFeedbackAttachment') throw new Error('persistence failed')
      return undefined
    })
    const firstRead = vi.fn(async () => new Uint8Array([1]).buffer)
    const secondRead = vi.fn(async () => new Uint8Array([2]).buffer)
    const firstDispose = vi.fn(async () => undefined)
    const secondDispose = vi.fn(async () => undefined)
    const first = defineAttachmentCandidate({
      id: 'candidate-1',
      source: 'file-input',
      fileName: 'first.txt',
      mediaType: 'text/plain',
      byteLength: 1,
      readBytes: firstRead,
      dispose: firstDispose,
    })
    const second = defineAttachmentCandidate({
      id: 'candidate-2',
      source: 'file-input',
      fileName: 'second.txt',
      mediaType: 'text/plain',
      byteLength: 1,
      readBytes: secondRead,
      dispose: secondDispose,
    })

    await createAttachmentController(context).persistAttachmentCandidates(
      { requestId: 'request-1', action: null },
      [first, second],
    )
    await Promise.all([first.dispose(), second.dispose()])

    expect(firstRead).toHaveBeenCalledOnce()
    expect(secondRead).not.toHaveBeenCalled()
    expect(firstDispose).toHaveBeenCalledOnce()
    expect(secondDispose).toHaveBeenCalledOnce()
    expect(get(session).message).toBe('Error: persistence failed')
  })

  it.each(['remove', 'reorder'] as const)(
    'does not apply a late %s result to a newly active workspace',
    async (operation) => {
      const attachment = {
        attachment_id: 'att-1',
        file_name: 'notes.txt',
        media_type: 'text/plain',
      }
      const original = {
        request: { request_id: 'request-1' },
        attachments: [
          attachment,
          { attachment_id: 'att-2', file_name: 'other.txt', media_type: 'text/plain' },
        ],
        draft: { saved_revision: 1 },
      }
      const switched = {
        request: { request_id: 'request-2' },
        attachments: [],
        draft: { saved_revision: 8 },
      }
      const result = {
        ...original,
        attachments: operation === 'remove' ? original.attachments.slice(1) : [...original.attachments].reverse(),
        draft: { saved_revision: 2 },
      }
      let visible = original
      let resolveOperation: ((workspace: typeof result) => void) | undefined
      const operationResult = new Promise<typeof result>((resolve) => (resolveOperation = resolve))
      const { context, session } = controllerContext()
      context.getWorkspace = () => visible as never
      context.getEditor = () => ({ removeAttachmentReference: vi.fn() }) as never
      mocks.applicationCall.mockImplementation(async (command: string) => {
        if (command === 'removeFeedbackAttachment' || command === 'reorderFeedbackAttachments') {
          return operationResult
        }
        return undefined
      })
      const controller = createAttachmentController(context)

      const pending =
        operation === 'remove'
          ? controller.removeAttachment(attachment as never)
          : controller.moveAttachment(0, 1)
      await Promise.resolve()
      await Promise.resolve()
      visible = switched
      resolveOperation?.(result)
      await pending

      expect(context.applyWorkspaceMutation).not.toHaveBeenCalled()
      expect(get(session).previews).toEqual({})
    },
  )
})

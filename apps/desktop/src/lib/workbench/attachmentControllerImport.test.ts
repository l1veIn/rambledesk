import { get } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { createAttachmentController } from './attachmentController'
import {
  controllerContext,
  fileCandidate,
  mocks,
  resetAttachmentMocks,
  unavailableCapabilities,
} from './attachmentControllerTestHarness'

describe('attachmentController import paths', () => {
  beforeEach(() => {
    resetAttachmentMocks()
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('never registers a global paste listener', () => {
    const { context, session } = controllerContext()
    const dispose = createAttachmentController(context).mount()

    expect(window.addEventListener).not.toHaveBeenCalledWith('paste', expect.any(Function))
    dispose()
  })


  it('subscribes and imports file drops when server paths are available without screen capture', async () => {
    const workspace = {
      request: { request_id: 'request-1', status: 'in_progress' },
      attachments: [],
      draft: { saved_revision: 1 },
    }
    const inserted = {
      ...workspace,
      draft: { saved_revision: 2 },
      attachments: [
        { attachment_id: 'att-1', file_name: 'notes.txt', media_type: 'text/plain' },
      ],
    }
    const { context, session } = controllerContext()
    context.capabilities = {
      ...context.capabilities,
      screenCapture: unavailableCapabilities.screenCapture,
    }
    context.getWorkspace = () => workspace as never
    mocks.applicationCall.mockImplementation(async (operation: string) => {
      if (operation === 'getFeedbackWorkspace') return workspace
      return undefined
    })
    mocks.importAttachmentPath.mockResolvedValue(inserted)

    const dispose = createAttachmentController(context).mount()
    expect(mocks.listeners.has('screen-capture-ready')).toBe(false)
    expect(mocks.listeners.has('file-drop')).toBe(true)
    mocks.listeners.get('file-drop')?.({ type: 'drop', paths: ['/tmp/notes.txt'] })

    await vi.waitFor(() => {
      expect(mocks.importAttachmentPath).toHaveBeenCalledWith({
        requestId: 'request-1',
        path: '/tmp/notes.txt',
        expectedRevision: 1,
      })
    })
    dispose()
  })

  it.each([
    ['missing workspace', null, false, false],
    ['completed workspace', 'completed', false, false],
    ['cancelled workspace', 'cancelled', false, false],
    ['locked workspace', 'in_progress', true, false],
    ['busy workspace', 'in_progress', false, true],
  ] as const)('rejects pasted files synchronously for a %s', (_label, status, locked, busy) => {
    const { context, session } = controllerContext()
    context.getWorkspace = () => status === null
      ? null
      : ({ request: { request_id: 'request-1', status }, attachments: [], draft: { saved_revision: 1 } }) as never
    context.getInteractionLocked = () => locked
    context.session.busy = () => busy
    const candidate = fileCandidate({
      fileName: 'screen.png',
      byteLength: 1,
      readBytes: async () => new Uint8Array([1]).buffer,
    })

    expect(createAttachmentController(context).acceptAttachmentCandidates([candidate])).toBe(false)
    expect(context.saveDraftNow).not.toHaveBeenCalled()
  })


  it('accepts an editable pasted file synchronously and reuses the attachment upload flow', async () => {
    const workspace = {
      request: { request_id: 'request-1', status: 'in_progress' },
      attachments: [],
      draft: { saved_revision: 3 },
    }
    const inserted = {
      ...workspace,
      draft: { saved_revision: 4 },
      attachments: [
        { attachment_id: 'att-1', file_name: 'screen.png', media_type: 'application/octet-stream' },
      ],
    }
    const { context, session } = controllerContext()
    context.getWorkspace = () => workspace as never
    mocks.applicationCall.mockImplementation(async (command: string) => {
      if (command === 'getFeedbackWorkspace') return workspace
      if (command === 'addFeedbackAttachment') return inserted
      return undefined
    })
    const candidate = fileCandidate({
      fileName: 'screen.png',
      byteLength: 3,
      readBytes: async () => new Uint8Array([1, 2, 3]).buffer,
    })
    const rejectedDispose = vi.fn(async () => undefined)
    const rejected = fileCandidate({
      fileName: 'rejected.png',
      byteLength: 1,
      readBytes: async () => new Uint8Array([4]).buffer,
      dispose: rejectedDispose,
    })
    const controller = createAttachmentController(context)

    expect(controller.acceptAttachmentCandidates([candidate])).toBe(true)
    expect(controller.acceptAttachmentCandidates([rejected])).toBe(false)

    await vi.waitFor(() => {
      expect(mocks.applicationCall).toHaveBeenCalledWith('addFeedbackAttachment', {
        request_id: 'request-1',
        file_name: 'screen.png',
        contents: new Uint8Array([1, 2, 3]).buffer,
        expected_revision: 3,
      })
    })
    await vi.waitFor(() => expect(context.routeDraftOperation).toHaveBeenCalled())
    await vi.waitFor(() => expect(rejectedDispose).toHaveBeenCalledOnce())
  })


  it('resets file input immediately and persists it as a file-input candidate', async () => {
    const workspace = {
      request: { request_id: 'request-1', status: 'in_progress' },
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
    const { context, session } = controllerContext()
    context.getWorkspace = () => workspace as never
    mocks.applicationCall.mockImplementation(async (command: string) => {
      if (command === 'getFeedbackWorkspace') return workspace
      if (command === 'addFeedbackAttachment') return inserted
      return undefined
    })
    const file = {
      name: 'notes.txt',
      type: 'text/plain',
      size: 4,
      arrayBuffer: vi.fn(async () => new Uint8Array([1, 2, 3, 4]).buffer),
    }
    const input = { files: [file], value: '/fake/notes.txt' }

    createAttachmentController(context).handleFileSelection({ currentTarget: input } as never)

    expect(input.value).toBe('')
    await vi.waitFor(() => {
      expect(mocks.applicationCall).toHaveBeenCalledWith(
        'addFeedbackAttachment',
        expect.objectContaining({ file_name: 'notes.txt' }),
      )
    })
    expect(context.recordAttachmentDiagnostic).not.toHaveBeenCalled()
  })


  it('allows exactly 20 MiB and rejects larger client files before reading bytes', async () => {
    const workspace = {
      request: { request_id: 'request-1', status: 'in_progress' },
      attachments: [],
      draft: { saved_revision: 3 },
    }
    const { context, session } = controllerContext()
    context.getWorkspace = () => workspace as never
    context.tr = (source, values) => source.replace('{name}', String(values?.name ?? ''))
    mocks.applicationCall.mockImplementation(async (command: string) => {
      if (command === 'getFeedbackWorkspace' || command === 'addFeedbackAttachment') {
        return workspace
      }
      return undefined
    })
    const exactRead = vi.fn(async () => new ArrayBuffer(0))
    const tooLargeRead = vi.fn(async () => new ArrayBuffer(0))
    const controller = createAttachmentController(context)

    await controller.importAttachmentCandidates([fileCandidate({
      fileName: 'exact.png',
      byteLength: 20 * 1024 * 1024,
      readBytes: exactRead,
    })])
    expect(exactRead).toHaveBeenCalledOnce()
    expect(mocks.applicationCall).toHaveBeenCalledWith(
      'addFeedbackAttachment',
      expect.objectContaining({ file_name: 'exact.png' }),
    )

    mocks.applicationCall.mockClear()
    await controller.importAttachmentCandidates([fileCandidate({
      fileName: 'too-large.png',
      byteLength: 20 * 1024 * 1024 + 1,
      readBytes: tooLargeRead,
    })])
    expect(tooLargeRead).not.toHaveBeenCalled()
    expect(mocks.applicationCall).not.toHaveBeenCalledWith(
      'addFeedbackAttachment',
      expect.anything(),
    )
    expect(get(session).message).toContain('too-large.png exceeds the 20 MiB limit')
    expect(get(session).tone).toBe('error')
  })

})

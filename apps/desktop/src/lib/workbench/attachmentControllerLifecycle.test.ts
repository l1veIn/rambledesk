import { get } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { FeedbackWorkspaceView } from '../feedback'
import { createAttachmentController } from './attachmentController'
import { controllerContext, fileCandidate, mocks, resetAttachmentMocks } from './attachmentControllerTestHarness'

function workspace(id = 'request-1', status = 'in_progress'): FeedbackWorkspaceView {
  return {
    request: { request_id: id, status },
    draft: { saved_revision: 1 }, attachments: [],
  } as unknown as FeedbackWorkspaceView
}

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => { resolve = done })
  return { promise, resolve }
}

function candidate(name = 'evidence.txt') {
  return fileCandidate({ fileName: name, byteLength: 1, readBytes: async () => new ArrayBuffer(1) })
}

function added(current: FeedbackWorkspaceView): FeedbackWorkspaceView {
  return { ...current, draft: { ...current.draft, saved_revision: current.draft.saved_revision + 1 },
    attachments: [...current.attachments, { attachment_id: `att-${current.draft.saved_revision}`, file_name: 'evidence.txt', media_type: 'text/plain' } as never] }
}

describe('attachment persistence lifecycle', () => {
  beforeEach(resetAttachmentMocks)
  afterEach(() => vi.unstubAllGlobals())

  it('rejects a terminal target before reading or persisting candidate bytes', async () => {
    const current = workspace('request-1', 'completed')
    const { context } = controllerContext()
    context.getWorkspace = () => current
    mocks.applicationCall.mockResolvedValue(current)
    const readBytes = vi.fn(async () => new ArrayBuffer(1))
    const dispose = vi.fn(async () => {})

    const result = await createAttachmentController(context).persistAttachmentCandidates(
      { requestId: 'request-1', action: null },
      [fileCandidate({ fileName: 'late.txt', byteLength: 1, readBytes, dispose })],
    )

    expect(result).toBe(false)
    expect(readBytes).not.toHaveBeenCalled()
    expect(dispose).toHaveBeenCalledOnce()
    expect(context.routeDraftOperation).not.toHaveBeenCalled()
  })

  it('keeps the first imported attachment and document insertion when a later upload fails', async () => {
    let current = workspace()
    const { context, session } = controllerContext()
    context.getWorkspace = () => current
    context.applyWorkspaceMutation = vi.fn((next) => { current = next })
    let uploads = 0
    mocks.applicationCall.mockImplementation(async (name) => {
      if (name === 'getFeedbackWorkspace') return current
      if (name === 'addFeedbackAttachment') {
        if (++uploads === 2) throw new Error('Second upload failed')
        current = added(current)
        return current
      }
    })

    const result = await createAttachmentController(context).persistAttachmentCandidates(
      { requestId: 'request-1', action: null }, [candidate('first.txt'), candidate('second.txt')],
    )

    expect(result).toBe(false)
    expect(context.routeDraftOperation).toHaveBeenCalledOnce()
    expect(context.applyWorkspaceMutation).toHaveBeenCalledWith(current)
    expect(current.attachments).toHaveLength(1)
    expect(get(session).message).toContain('Second upload failed')
  })

  it('refetches an upload with a lost response without replaying or guessing its document insertion', async () => {
    let backend = workspace()
    const visible = backend
    const { context, session } = controllerContext()
    context.getWorkspace = () => visible
    mocks.applicationCall.mockImplementation(async (name) => {
      if (name === 'getFeedbackWorkspace') return backend
      if (name === 'addFeedbackAttachment') {
        backend = added(backend)
        throw new Error('Upload response lost')
      }
    })

    expect(await createAttachmentController(context).persistAttachmentCandidates(
      { requestId: 'request-1', action: null }, [candidate()],
    )).toBe(false)

    expect(mocks.applicationCall.mock.calls.filter(([name]) => name === 'addFeedbackAttachment')).toHaveLength(1)
    expect(context.applyWorkspaceMutation).toHaveBeenCalledWith(backend)
    expect(context.routeDraftOperation).not.toHaveBeenCalled()
    expect(get(session).tone).toBe('error')
  })

  it('disposes a candidate without mutating after the owning controller is destroyed', async () => {
    const bytes = deferred<ArrayBuffer>()
    const readBytes = vi.fn(() => bytes.promise)
    const dispose = vi.fn(async () => {})
    const { context, session } = controllerContext()
    context.getWorkspace = () => workspace()
    mocks.applicationCall.mockResolvedValue(workspace())
    const controller = createAttachmentController(context)
    const cleanup = controller.mount()
    const pending = controller.persistAttachmentCandidates({ requestId: 'request-1', action: null }, [
      fileCandidate({ fileName: 'late.txt', byteLength: 1, readBytes, dispose }),
    ])
    await vi.waitFor(() => expect(readBytes).toHaveBeenCalled())
    cleanup()
    bytes.resolve(new ArrayBuffer(1))

    expect(await pending).toBe(false)
    expect(mocks.applicationCall.mock.calls.some(([name]) => name === 'addFeedbackAttachment')).toBe(false)
    expect(context.routeDraftOperation).not.toHaveBeenCalled()
    expect(dispose).toHaveBeenCalledOnce()
    expect(get(session).busy).toBe(false)
  })

  it.each(['remove', 'reorder'] as const)('does not use a new workspace revision for a pending %s', async (operation) => {
    let current = added(added(workspace()))
    const { context } = controllerContext()
    context.getWorkspace = () => current
    context.getEditor = () => ({ removeAttachmentReference: vi.fn() }) as never
    context.getSavedRevision = () => current.draft.saved_revision
    context.saveDraftNow = async () => { current = workspace('request-2'); return true }
    mocks.applicationCall.mockResolvedValue(workspace())
    const controller = createAttachmentController(context)

    if (operation === 'remove') await controller.removeAttachment(current.attachments[0])
    else await controller.moveAttachment(0, 1)

    expect(mocks.applicationCall).not.toHaveBeenCalled()
  })

  it('reserves screen capture before awaiting draft preparation', async () => {
    const saved = deferred<boolean>()
    const { context } = controllerContext()
    context.getWorkspace = () => workspace()
    context.saveDraftNow = () => saved.promise
    const controller = createAttachmentController(context)
    const first = controller.startScreenCapture()
    const second = controller.startScreenCapture()
    saved.resolve(true)
    await Promise.all([first, second])

    expect(mocks.beginCapture).toHaveBeenCalledOnce()
  })

  it('uses the revision saved by each document insertion for the following upload', async () => {
    let current = workspace()
    const { context } = controllerContext()
    context.getWorkspace = () => current
    context.applyWorkspaceMutation = (next) => { current = next }
    context.routeDraftOperation = vi.fn(async () => {
      current = { ...current, draft: { ...current.draft, saved_revision: current.draft.saved_revision + 1 } }
    })
    mocks.applicationCall.mockImplementation(async (name) => {
      if (name === 'getFeedbackWorkspace') return current
      if (name === 'addFeedbackAttachment') { current = added(current); return current }
    })

    expect(await createAttachmentController(context).persistAttachmentCandidates(
      { requestId: 'request-1', action: null }, [candidate('first.txt'), candidate('second.txt')],
    )).toBe(true)
    expect(mocks.applicationCall.mock.calls.filter(([name]) => name === 'addFeedbackAttachment')
      .map(([, input]) => input.expected_revision)).toEqual([1, 3])
    expect(context.routeDraftOperation).toHaveBeenCalledTimes(2)
  })

  it('queues a late captured candidate behind an in-flight removal and retains busy ownership', async () => {
    let current = added(workspace())
    const removed = deferred<FeedbackWorkspaceView>()
    const uploaded = deferred<FeedbackWorkspaceView>()
    const { context, session } = controllerContext()
    context.getWorkspace = () => current
    context.getSavedRevision = () => current.draft.saved_revision
    context.getEditor = () => ({ removeAttachmentReference: vi.fn() }) as never
    context.applyWorkspaceMutation = (next) => { current = next }
    mocks.applicationCall.mockImplementation(async (name) => {
      if (name === 'getFeedbackWorkspace') return current
      if (name === 'removeFeedbackAttachment') return removed.promise
      if (name === 'addFeedbackAttachment') return uploaded.promise
    })
    const controller = createAttachmentController(context)
    const removing = controller.removeAttachment(current.attachments[0])
    await vi.waitFor(() => expect(mocks.applicationCall).toHaveBeenCalledWith('removeFeedbackAttachment', expect.anything()))
    const importing = controller.persistAttachmentCandidates({ requestId: 'request-1', action: null }, [candidate()])
    await Promise.resolve()
    expect(mocks.applicationCall.mock.calls.some(([name]) => name === 'addFeedbackAttachment')).toBe(false)

    current = { ...current, attachments: [], draft: { ...current.draft, saved_revision: 3 } }
    removed.resolve(current)
    await removing
    await vi.waitFor(() => expect(mocks.applicationCall).toHaveBeenCalledWith('addFeedbackAttachment', expect.objectContaining({ expected_revision: 3 })))
    expect(get(session).busy).toBe(true)
    uploaded.resolve(added(current))
    expect(await importing).toBe(true)
    expect(get(session).busy).toBe(false)
  })

  it('does not hold a persisted document receipt behind an optional image preview read', async () => {
    const preview = deferred<ArrayBuffer>()
    const current = workspace()
    const inserted = added(current)
    inserted.attachments[0].media_type = 'image/png'
    const { context } = controllerContext()
    context.getWorkspace = () => current
    mocks.applicationCall.mockImplementation(async (name) => {
      if (name === 'getFeedbackWorkspace') return current
      if (name === 'addFeedbackAttachment') return inserted
      if (name === 'readFeedbackAttachment') return preview.promise
    })
    const controller = createAttachmentController(context)
    let result: boolean | undefined
    const operation = controller.persistAttachmentCandidates({ requestId: 'request-1', action: null }, [candidate()])
      .then((value) => { result = value })
    await vi.waitFor(() => expect(result).toBe(true))
    expect(context.routeDraftOperation).toHaveBeenCalledOnce()
    controller.releasePreviews()
    preview.resolve(new ArrayBuffer(1))
    await operation
  })
})

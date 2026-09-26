import { get } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { FeedbackWorkspaceView } from '../feedback'
import { createAttachmentController } from './attachmentController'
import { controllerContext, fileCandidate, mocks, resetAttachmentMocks, screenCandidate } from './attachmentControllerTestHarness'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => { resolve = done })
  return { promise, resolve }
}

function workspace(requestId = 'request-1'): FeedbackWorkspaceView {
  return { request: { request_id: requestId, status: 'in_progress' }, draft: { saved_revision: 1 }, attachments: [] } as unknown as FeedbackWorkspaceView
}

function setup() {
  let current = workspace()
  const { context, session } = controllerContext()
  context.getWorkspace = () => current
  context.applyWorkspaceMutation = (next) => { current = next }
  mocks.applicationCall.mockImplementation(async (command) => {
    if (command === 'getFeedbackWorkspace') return current
    if (command === 'addFeedbackAttachment') {
      return { ...current, draft: { ...current.draft, saved_revision: 2 }, attachments: [{ attachment_id: 'attachment-1', file_name: 'shot.png', media_type: 'image/png' }] }
    }
    return new ArrayBuffer(0)
  })
  const controller = createAttachmentController(context)
  return { context, session, controller }
}

describe('request attachment preparation', () => {
  beforeEach(resetAttachmentMocks)
  afterEach(() => vi.unstubAllGlobals())

  it('returns pending-capture immediately while the human capture window is open', async () => {
    const { controller, session } = setup()
    const cleanup = controller.mount()
    await controller.startScreenCapture()

    expect(await controller.prepareFeedback('request-1')).toEqual({ kind: 'pending-capture' })
    expect(get(session).mediaBusy).toBe(true)
    expect(get(session).message).toContain('Finish or cancel')
    expect(await controller.prepareFeedback('request-2')).toEqual({ kind: 'ready' })

    mocks.listeners.get('screen-capture-finished')?.({ outcome: 'cancelled', candidateId: null })
    expect(await controller.prepareFeedback('request-1')).toEqual({ kind: 'ready' })
    expect(get(session).mediaBusy).toBe(false)
    cleanup()
  })

  it('also blocks while native capture is still starting', async () => {
    const begin = deferred<void>()
    mocks.beginCapture.mockReturnValue(begin.promise)
    const { controller } = setup()
    const starting = controller.startScreenCapture()
    expect(await controller.prepareFeedback('request-1')).toEqual({ kind: 'pending-capture' })
    begin.resolve()
    await starting
  })

  it('waits for upload, document insertion and cleanup even if capture closes meanwhile', async () => {
    const insertion = deferred<void>()
    const disposal = deferred<void>()
    const { controller, context, session } = setup()
    context.routeDraftOperation = vi.fn(() => insertion.promise)
    mocks.discardCapture.mockReturnValue(disposal.promise)
    const cleanup = controller.mount()
    await controller.startScreenCapture()
    mocks.listeners.get('screen-capture-ready')?.(screenCandidate())
    await vi.waitFor(() => expect(context.routeDraftOperation).toHaveBeenCalledOnce())
    mocks.listeners.get('screen-capture-finished')?.({ outcome: 'pinned', candidateId: 'capture-1' })
    let prepared = false
    const preparing = controller.prepareFeedback('request-1').then((result) => { prepared = true; return result })
    await Promise.resolve()
    expect(prepared).toBe(false)
    expect(get(session).mediaBusy).toBe(true)
    insertion.resolve()
    await vi.waitFor(() => expect(mocks.discardCapture).toHaveBeenCalledOnce())
    expect(prepared).toBe(false)
    disposal.resolve()
    expect(await preparing).toEqual({ kind: 'ready' })
    expect(get(session).mediaBusy).toBe(false)
    cleanup()
  })

  it('joins another accepted write queued while preparation is draining', async () => {
    const first = deferred<ArrayBuffer>()
    const second = deferred<ArrayBuffer>()
    const { controller } = setup()
    const write = (name: string, bytes: Promise<ArrayBuffer>) => controller.persistAttachmentCandidates(
      { requestId: 'request-1', action: null },
      [fileCandidate({ fileName: name, byteLength: 1, readBytes: () => bytes })],
    )
    const firstWrite = write('first.png', first.promise)
    let prepared = false
    const preparing = controller.prepareFeedback('request-1').then((result) => { prepared = true; return result })
    const secondWrite = write('second.png', second.promise)
    first.resolve(new ArrayBuffer(1))
    await firstWrite
    expect(prepared).toBe(false)
    second.resolve(new ArrayBuffer(1))
    await secondWrite
    expect(await preparing).toEqual({ kind: 'ready' })
  })

  it('reports an accepted upload failure instead of preparing a partial result', async () => {
    const bytes = deferred<ArrayBuffer>()
    const { controller, context } = setup()
    const read = vi.fn(async () => { await bytes.promise; throw new Error('File read failed') })
    const importing = controller.persistAttachmentCandidates({ requestId: 'request-1', action: null }, [
      fileCandidate({ fileName: 'broken.png', byteLength: 1, readBytes: read }),
    ])
    const preparing = controller.prepareFeedback('request-1')
    bytes.resolve(new ArrayBuffer(1))

    expect(await importing).toBe(false)
    expect(await preparing).toEqual({ kind: 'failed', message: expect.stringContaining('File read failed') })
    expect(context.routeDraftOperation).not.toHaveBeenCalled()
  })

  it('reports a failure that settled before preparation once, then permits an explicit retry', async () => {
    const { controller } = setup()
    expect(await controller.persistAttachmentCandidates({ requestId: 'request-1', action: null }, [
      fileCandidate({ fileName: 'missing.png', byteLength: 1, readBytes: async () => { throw new Error('File missing') } }),
    ])).toBe(false)

    expect(await controller.prepareFeedback('request-2')).toEqual({ kind: 'ready' })
    expect(await controller.prepareFeedback('request-1')).toEqual({ kind: 'failed', message: expect.stringContaining('submit again to continue') })
    expect(await controller.prepareFeedback('request-1')).toEqual({ kind: 'ready' })
  })

  it('does not turn an unmounted owner into a successful preparation', async () => {
    const bytes = deferred<ArrayBuffer>()
    const { controller, context } = setup()
    const read = vi.fn(() => bytes.promise)
    const cleanup = controller.mount()
    const importing = controller.persistAttachmentCandidates({ requestId: 'request-1', action: null }, [
      fileCandidate({ fileName: 'late.png', byteLength: 1, readBytes: read }),
    ])
    await vi.waitFor(() => expect(read).toHaveBeenCalledOnce())
    const preparing = controller.prepareFeedback('request-1')
    cleanup()
    bytes.resolve(new ArrayBuffer(1))

    expect(await importing).toBe(false)
    expect(await preparing).toMatchObject({ kind: 'failed' })
    expect(await controller.prepareFeedback('request-1')).toMatchObject({ kind: 'failed' })
    expect(context.routeDraftOperation).not.toHaveBeenCalled()
  })

  it('rejects late uploads after the current request freezes', async () => {
    const { controller, context } = setup()
    context.getInteractionLocked = () => true
    const readBytes = vi.fn(async () => new ArrayBuffer(1))
    const dispose = vi.fn(async () => {})
    expect(await controller.persistAttachmentCandidates({ requestId: 'request-1', action: null }, [
      fileCandidate({ fileName: 'late.png', byteLength: 1, readBytes, dispose }),
    ])).toBe(false)
    expect(readBytes).not.toHaveBeenCalled()
    expect(dispose).toHaveBeenCalledOnce()
    expect(mocks.applicationCall).not.toHaveBeenCalled()
  })

  it('preserves a capture target when another request becomes visible and locked', async () => {
    const { controller, context } = setup()
    const action = { actionId: 'original', actionIndex: 0, title: 'Original action' }
    context.activeActionFor = () => action
    const cleanup = controller.mount()
    await controller.startScreenCapture()
    context.getWorkspace = () => workspace('request-2')
    context.getInteractionLocked = () => true
    mocks.listeners.get('screen-capture-ready')?.(screenCandidate())
    expect(await controller.prepareFeedback('request-1')).toEqual({ kind: 'ready' })
    expect(context.routeDraftOperation).toHaveBeenCalledWith('request-1', expect.objectContaining({ action }))
    cleanup()
  })

  it('includes inserting an existing attachment reference in the request barrier', async () => {
    const insertion = deferred<void>()
    const { controller, context } = setup()
    context.routeDraftOperation = vi.fn(() => insertion.promise)
    controller.insertExistingAttachment({ attachment_id: 'existing', file_name: 'old.png' } as never)
    let prepared = false
    const preparing = controller.settled('request-1').then((result) => { prepared = true; return result })
    await vi.waitFor(() => expect(context.routeDraftOperation).toHaveBeenCalledOnce())
    expect(prepared).toBe(false)
    insertion.resolve()
    expect(await preparing).toEqual({ kind: 'ready' })
  })
})

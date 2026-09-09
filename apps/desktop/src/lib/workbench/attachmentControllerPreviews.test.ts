import { get } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { createAttachmentController } from './attachmentController'
import { controllerContext, mocks, resetAttachmentMocks } from './attachmentControllerTestHarness'
import type { FeedbackWorkspaceView } from '../feedback'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => { resolve = done })
  return { promise, resolve }
}

function workspace(ids: string[]): FeedbackWorkspaceView {
  return {
    request: { request_id: 'request-1', status: 'in_progress' },
    attachments: ids.map((id) => ({ attachment_id: id, media_type: 'image/png' })),
    draft: { saved_revision: 1 },
  } as FeedbackWorkspaceView
}

describe('attachment preview URL ownership', () => {
  beforeEach(() => {
    resetAttachmentMocks()
    let sequence = 0
    vi.stubGlobal('URL', {
      createObjectURL: vi.fn(() => `blob:preview-${++sequence}`),
      revokeObjectURL: vi.fn(),
    })
  })
  afterEach(() => vi.unstubAllGlobals())

  it('keeps the newest attachment projection when an earlier refresh finishes late', async () => {
    const late = deferred<ArrayBuffer>()
    const { context, session } = controllerContext()
    context.getWorkspace = () => workspace(['a'])
    mocks.applicationCall.mockResolvedValueOnce(new ArrayBuffer(1))
    const controller = createAttachmentController(context)
    await controller.refreshPreviews(workspace(['a']))
    const retained = get(session).previews.a

    mocks.applicationCall.mockReturnValueOnce(late.promise)
    const oldRefresh = controller.refreshPreviews(workspace(['a', 'b']))
    await controller.refreshPreviews(workspace(['a']))
    late.resolve(new ArrayBuffer(1))
    await oldRefresh

    expect(get(session).previews).toEqual({ a: retained })
    expect(URL.revokeObjectURL).not.toHaveBeenCalledWith(retained)
    controller.releasePreviews()
  })

  it('does not resurrect a preview after release while a read is pending', async () => {
    const late = deferred<ArrayBuffer>()
    const { context, session } = controllerContext()
    context.getWorkspace = () => workspace(['a'])
    mocks.applicationCall.mockReturnValueOnce(late.promise)
    const controller = createAttachmentController(context)
    const pending = controller.refreshPreviews(workspace(['a']))
    controller.releasePreviews()
    late.resolve(new ArrayBuffer(1))
    await pending

    expect(get(session).previews).toEqual({})
    expect(URL.createObjectURL).not.toHaveBeenCalled()
  })

  it('publishes removal before revoking the last editor URL', async () => {
    const { context, session } = controllerContext()
    context.getWorkspace = () => workspace(['a'])
    mocks.applicationCall.mockResolvedValue(new ArrayBuffer(1))
    const controller = createAttachmentController(context)
    await controller.refreshPreviews(workspace(['a']))
    const url = get(session).previews.a
    vi.mocked(URL.revokeObjectURL).mockImplementation(() => {
      expect(Object.values(get(session).previews)).not.toContain(url)
    })

    await controller.refreshPreviews(workspace([]))
    expect(URL.revokeObjectURL).toHaveBeenCalledOnce()
    expect(URL.revokeObjectURL).toHaveBeenCalledWith(url)
  })
})

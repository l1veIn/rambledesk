import { get } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { createAttachmentController } from './attachmentController'
import {
  controllerContext,
  mocks,
  resetAttachmentMocks,
  screenCandidate,
} from './attachmentControllerTestHarness'

describe('attachmentController capture lifecycle', () => {
  beforeEach(() => {
    resetAttachmentMocks()
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('keeps capture busy until the capture finishes and blocks duplicate starts', async () => {
    const { context, session } = controllerContext()
    const controller = createAttachmentController(context)

    await controller.startScreenCapture()
    await controller.startScreenCapture()

    expect(mocks.beginCapture).toHaveBeenCalledTimes(1)
    expect(get(session).captureBusy).toBe(true)
  })


  it('starts capture before Ramble when the workspace supplies the request id', async () => {
    const { context, session } = controllerContext()
    context.getRambleRequestId = () => ''
    context.getWorkspace = () => ({
      request: { request_id: 'workspace-request' },
      attachments: [],
      draft: { saved_revision: 1 },
    }) as never
    const controller = createAttachmentController(context)

    await controller.startScreenCapture()

    expect(mocks.beginCapture).toHaveBeenCalledTimes(1)
    expect(context.saveDraftNow).toHaveBeenCalled()
  })


  it('locks a toolbar capture to the visible workspace instead of a background Ramble', async () => {
    const workspace = {
      request: { request_id: 'request-b', status: 'in_progress' },
      attachments: [] as Array<{ attachment_id: string; file_name: string; media_type: string }>,
      draft: { saved_revision: 4 },
    }
    const inserted = {
      ...workspace,
      draft: { saved_revision: 5 },
      attachments: [{ attachment_id: 'att-b', file_name: 'shot.png', media_type: 'image/png' }],
    }
    const { context, session } = controllerContext()
    context.getWorkspace = () => workspace as never
    context.getRambleRequestId = () => 'request-a'
    context.activeActionFor = ((requestId: string) => ({
      actionId: `action-${requestId}`,
      actionIndex: 0,
      title: requestId,
    })) as never
    mocks.applicationCall.mockImplementation(async (command: string, input: { request_id?: string }) => {
      expect(input.request_id).toBe('request-b')
      if (command === 'getFeedbackWorkspace') return workspace
      if (command === 'addFeedbackAttachment') return inserted
      return new ArrayBuffer(0)
    })

    const controller = createAttachmentController(context)
    const cleanup = controller.mount()
    await controller.startScreenCapture()
    mocks.listeners.get('screen-capture-ready')?.(screenCandidate())

    await vi.waitFor(() => expect(context.routeDraftOperation).toHaveBeenCalled())
    expect(context.routeDraftOperation).toHaveBeenCalledWith(
      'request-b',
      expect.objectContaining({
        action: {
          actionId: 'action-request-b',
          actionIndex: 0,
          title: 'request-b',
        },
      }),
    )
    cleanup()
  })


  it('clears capture busy without changing attachment busy on cancel or pin', async () => {
    const { context, session } = controllerContext()
    const controller = createAttachmentController(context)
    const cleanup = controller.mount()
    await vi.waitFor(() => expect(mocks.listeners.has('screen-capture-finished')).toBe(true))

    mocks.listeners.get('screen-capture-finished')?.({
      candidateId: 'capture-1', outcome: 'cancelled',
    })

    expect(get(session).captureBusy).toBe(false)
    expect(get(session).busy).toBe(false)
    cleanup()
  })


  it('toasts only the capture result, not an in-progress insert', async () => {
    const workspace = {
      request: { request_id: 'request-1' },
      attachments: [] as Array<{ attachment_id: string; file_name: string; media_type: string }>,
      draft: { saved_revision: 1 },
    }
    const inserted = {
      ...workspace,
      attachments: [{ attachment_id: 'att-1', file_name: 'shot.png', media_type: 'image/png' }],
    }
    const { context, session } = controllerContext()
    context.getWorkspace = () => workspace as never
    context.getEditor = () => ({ insertAttachments: () => true }) as never
    vi.stubGlobal('URL', {
      createObjectURL: vi.fn(() => 'blob:preview'),
      revokeObjectURL: vi.fn(),
    })
    mocks.applicationCall.mockImplementation(async (command: string) => {
      if (command === 'getFeedbackWorkspace') return workspace
      if (command === 'addFeedbackAttachment') return inserted
      return new ArrayBuffer(0)
    })

    const controller = createAttachmentController(context)
    const cleanup = controller.mount()
    await vi.waitFor(() => expect(mocks.listeners.has('screen-capture-ready')).toBe(true))
    await controller.startScreenCapture()

    mocks.listeners.get('screen-capture-ready')?.(screenCandidate())

    await vi.waitFor(() => {
      expect(get(session).message).toBe('Capture inserted at the current document position')
      expect(get(session).tone).toBe('success')
    })
    expect(get(session).message).not.toBe('Inserting capture…')
    cleanup()
  })


  it('keeps the request Action selected when capture started', async () => {
    const workspace = {
      request: { request_id: 'request-1' },
      attachments: [] as Array<{ attachment_id: string; file_name: string; media_type: string }>,
      draft: { saved_revision: 1 },
    }
    const inserted = {
      ...workspace,
      draft: { saved_revision: 2 },
      attachments: [{ attachment_id: 'att-1', file_name: 'shot.png', media_type: 'image/png' }],
    }
    let action = { actionId: 'action-a', actionIndex: 0, title: 'First' }
    const { context, session } = controllerContext()
    context.getWorkspace = () => workspace as never
    context.activeActionFor = (() => action) as never
    vi.stubGlobal('URL', {
      createObjectURL: vi.fn(() => 'blob:preview'),
      revokeObjectURL: vi.fn(),
    })
    mocks.applicationCall.mockImplementation(async (command: string) => {
      if (command === 'getFeedbackWorkspace') return workspace
      if (command === 'addFeedbackAttachment') return inserted
      return new ArrayBuffer(0)
    })

    const controller = createAttachmentController(context)
    const cleanup = controller.mount()
    await vi.waitFor(() => expect(mocks.listeners.has('screen-capture-ready')).toBe(true))
    await controller.startScreenCapture()
    action = { actionId: 'action-b', actionIndex: 1, title: 'Second' }
    mocks.listeners.get('screen-capture-ready')?.(screenCandidate())

    await vi.waitFor(() => expect(context.routeDraftOperation).toHaveBeenCalled())
    expect(context.routeDraftOperation).toHaveBeenCalledWith(
      'request-1',
      expect.objectContaining({
        kind: 'appendAttachment',
        action: { actionId: 'action-a', actionIndex: 0, title: 'First' },
      }),
    )
    cleanup()
  })

})

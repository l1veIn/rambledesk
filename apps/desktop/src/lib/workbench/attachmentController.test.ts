import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  applicationCall: vi.fn(),
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
}))

vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (event: string, handler: (event: { payload: unknown }) => void) => {
    mocks.listeners.set(event, handler)
    return vi.fn()
  }),
}))
vi.mock('@tauri-apps/api/webview', () => ({
  getCurrentWebview: () => ({ onDragDropEvent: vi.fn(async () => vi.fn()) }),
}))

import { createAttachmentController } from './attachmentController'
import { TestApplicationTransport } from '../application/testApplicationTransport'

function controllerContext() {
  let captureBusy = false
  const setBusy = vi.fn()
  const setCaptureBusy = vi.fn((busy: boolean) => {
    captureBusy = busy
  })
  const transport = new TestApplicationTransport(undefined)
    .handle('getFeedbackWorkspace', (input) => mocks.applicationCall('getFeedbackWorkspace', input))
    .handle('addFeedbackAttachment', (input) => mocks.applicationCall('addFeedbackAttachment', input))
    .handle('removeFeedbackAttachment', (input) => mocks.applicationCall('removeFeedbackAttachment', input))
    .handle('reorderFeedbackAttachments', (input) => mocks.applicationCall('reorderFeedbackAttachments', input))
    .handle('readFeedbackAttachment', (input) => mocks.applicationCall('readFeedbackAttachment', input))
  return {
    context: {
      isTauri: true,
      transport,
      tr: (source: string) => source,
      messageFrom: (cause: unknown) => String(cause),
      getWorkspace: () => null,
      getEditor: () => undefined,
      getRambleRequestId: () => 'request-1',
      getInteractionLocked: () => false,
      getSavedRevision: () => 0,
      getBusy: () => false,
      getCaptureBusy: () => captureBusy,
      getPreviews: () => ({}),
      setBusy,
      setCaptureBusy,
      setMessage: vi.fn(),
      setPreviews: vi.fn(),
      setDragActive: vi.fn(),
      saveDraftNow: vi.fn(async () => true),
      waitForRambleMarkdown: vi.fn(async () => undefined),
      routeDraftOperation: vi.fn(async () => undefined),
      activeActionFor: () => null,
      applyWorkspaceMutation: vi.fn(),
    },
    setBusy,
    setCaptureBusy,
  }
}

describe('attachmentController screen capture state', () => {
  beforeEach(() => {
    mocks.invoke.mockReset()
    mocks.applicationCall.mockReset()
    mocks.listeners.clear()
    vi.stubGlobal('window', {
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('does not claim browser image-paste support before the browser capability lands', () => {
    const { context } = controllerContext()
    context.isTauri = false
    const dispose = createAttachmentController(context).mount()

    expect(window.addEventListener).not.toHaveBeenCalledWith('paste', expect.any(Function))
    dispose()
  })

  it('keeps capture busy until the capture finishes and blocks duplicate starts', async () => {
    mocks.invoke.mockResolvedValue(undefined)
    const { context, setCaptureBusy } = controllerContext()
    const controller = createAttachmentController(context)

    await controller.startScreenCapture()
    await controller.startScreenCapture()

    expect(mocks.invoke).toHaveBeenCalledTimes(1)
    expect(mocks.invoke).toHaveBeenCalledWith('begin_screen_capture')
    expect(setCaptureBusy).toHaveBeenCalledTimes(1)
    expect(setCaptureBusy).toHaveBeenLastCalledWith(true)
  })

  it('starts capture before Ramble when the workspace supplies the request id', async () => {
    mocks.invoke.mockResolvedValue(undefined)
    const { context } = controllerContext()
    context.getRambleRequestId = () => ''
    context.getWorkspace = () => ({
      request: { request_id: 'workspace-request' },
      attachments: [],
      draft: { saved_revision: 1 },
    }) as never
    const controller = createAttachmentController(context)

    await controller.startScreenCapture()

    expect(mocks.invoke).toHaveBeenCalledWith('begin_screen_capture')
    expect(context.saveDraftNow).toHaveBeenCalled()
  })

  it('clears capture busy without changing attachment busy on cancel or pin', async () => {
    const { context, setBusy, setCaptureBusy } = controllerContext()
    const controller = createAttachmentController(context)
    const cleanup = controller.mount()
    await vi.waitFor(() => expect(mocks.listeners.has('screen-capture-finished')).toBe(true))

    mocks.listeners.get('screen-capture-finished')?.({
      payload: { capture_session_id: 'capture-1', outcome: 'cancelled' },
    })

    expect(setCaptureBusy).toHaveBeenCalledWith(false)
    expect(setBusy).not.toHaveBeenCalled()
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
    const { context } = controllerContext()
    context.getWorkspace = () => workspace as never
    context.getEditor = () => ({ insertAttachments: () => true }) as never
    vi.stubGlobal('URL', {
      createObjectURL: vi.fn(() => 'blob:preview'),
      revokeObjectURL: vi.fn(),
    })
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'add_completed_screen_capture') return inserted
      return undefined
    })
    mocks.applicationCall.mockResolvedValue(new ArrayBuffer(0))

    const controller = createAttachmentController(context)
    const cleanup = controller.mount()
    await vi.waitFor(() => expect(mocks.listeners.has('screen-capture-ready')).toBe(true))

    mocks.listeners.get('screen-capture-ready')?.({
      payload: { capture_session_id: 'capture-1', file_name: 'shot.png' },
    })

    await vi.waitFor(() => {
      expect(context.setMessage).toHaveBeenCalledWith(
        'Capture inserted at the current document position',
        'success',
      )
    })
    expect(context.setMessage).not.toHaveBeenCalledWith('Inserting capture…', 'info')
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
    const { context } = controllerContext()
    context.getWorkspace = () => workspace as never
    context.activeActionFor = (() => action) as never
    vi.stubGlobal('URL', {
      createObjectURL: vi.fn(() => 'blob:preview'),
      revokeObjectURL: vi.fn(),
    })
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'add_completed_screen_capture') return inserted
      return undefined
    })
    mocks.applicationCall.mockResolvedValue(new ArrayBuffer(0))

    const controller = createAttachmentController(context)
    const cleanup = controller.mount()
    await vi.waitFor(() => expect(mocks.listeners.has('screen-capture-ready')).toBe(true))
    await controller.startScreenCapture()
    action = { actionId: 'action-b', actionIndex: 1, title: 'Second' }
    mocks.listeners.get('screen-capture-ready')?.({
      payload: { capture_session_id: 'capture-1', file_name: 'shot.png' },
    })

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
    const { context } = controllerContext()
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
    const file = {
      name: 'notes.txt',
      size: 4,
      arrayBuffer: async () => new Uint8Array([1, 2, 3, 4]).buffer,
    } as File

    await createAttachmentController(context).importFiles([file])

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
      const { context } = controllerContext()
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
      expect(context.setPreviews).not.toHaveBeenCalled()
    },
  )
})

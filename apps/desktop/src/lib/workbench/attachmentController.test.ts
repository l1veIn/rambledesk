import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
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

function controllerContext() {
  let captureBusy = false
  const setBusy = vi.fn()
  const setCaptureBusy = vi.fn((busy: boolean) => {
    captureBusy = busy
  })
  return {
    context: {
      isTauri: true,
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
      appendRambleMarkdown: vi.fn(async () => undefined),
      applyWorkspaceMutation: vi.fn(),
    },
    setBusy,
    setCaptureBusy,
  }
}

describe('attachmentController screen capture state', () => {
  beforeEach(() => {
    mocks.invoke.mockReset()
    mocks.listeners.clear()
    vi.stubGlobal('window', {
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })
  })

  afterEach(() => {
    vi.unstubAllGlobals()
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
      if (command === 'read_feedback_attachment') return new ArrayBuffer(0)
      return undefined
    })

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
})

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
      if (command === 'read_feedback_attachment') return new ArrayBuffer(0)
      return undefined
    })

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
      'ramble',
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
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'get_feedback_workspace') return target
      if (command === 'add_feedback_attachment') return inserted
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
      'workspace',
    )
    expect(context.applyWorkspaceMutation).not.toHaveBeenCalled()
  })

  it('resolves the Artifact port when the operation starts', async () => {
    const workspace = {
      request: { request_id: 'request-1' },
      attachments: [],
      draft: { saved_revision: 1 },
    }
    const inserted = {
      ...workspace,
      attachments: [{
        attachment_id: 'artifact-1',
        file_name: 'notes.txt',
        media_type: 'text/plain',
      }],
      draft: { saved_revision: 2 },
    }
    const { context } = controllerContext()
    context.getWorkspace = () => workspace as never
    let managed = false
    const port = {
      loadWorkspace: vi.fn(async () => workspace as never),
      addBytes: vi.fn(async () => inserted as never),
      addPath: vi.fn(),
      addScreenCapture: vi.fn(),
      remove: vi.fn(),
      reorder: vi.fn(),
      read: vi.fn(),
    }
    const controller = createAttachmentController({
      ...context,
      getArtifactPort: () => managed ? port as never : undefined,
    })
    managed = true

    await controller.importFiles([{
      name: 'notes.txt',
      type: 'text/plain',
      size: 4,
      arrayBuffer: async () => new Uint8Array([1, 2, 3, 4]).buffer,
    } as File])

    expect(port.loadWorkspace).toHaveBeenCalledWith('request-1')
    expect(port.addBytes).toHaveBeenCalledWith(expect.objectContaining({
      requestId: 'request-1',
      fileName: 'notes.txt',
    }))
    expect(mocks.invoke).not.toHaveBeenCalledWith('add_feedback_attachment', expect.anything())
  })

  it('keeps an active Ramble attachment on its owner when the visible request has the same raw id', async () => {
    const visibleWorkspace = {
      request: { request_id: 'same-id' },
      attachments: [],
      draft: { saved_revision: 9 },
    }
    const rambleWorkspace = {
      request: { request_id: 'same-id' },
      attachments: [],
      draft: { saved_revision: 2 },
    }
    const inserted = {
      ...rambleWorkspace,
      attachments: [{
        attachment_id: 'artifact-ramble',
        file_name: 'ramble.txt',
        media_type: 'text/plain',
      }],
      draft: { saved_revision: 3 },
    }
    const { context } = controllerContext()
    context.getWorkspace = () => visibleWorkspace as never
    const port = {
      loadWorkspace: vi.fn(async () => rambleWorkspace as never),
      addBytes: vi.fn(),
      addPath: vi.fn(async () => inserted as never),
      addScreenCapture: vi.fn(),
      remove: vi.fn(),
      reorder: vi.fn(),
      read: vi.fn(),
    }
    const getArtifactPort = vi.fn(() => port)
    const getActiveAction = vi.fn(() => null)

    await createAttachmentController({
      ...context,
      getRambleRequestId: () => 'same-id',
      isOperationTargetVisible: (_requestId, target) => target === 'workspace',
      getArtifactPort: getArtifactPort as never,
      activeActionFor: getActiveAction,
    }).importAttachmentPaths(['/tmp/ramble.txt'])

    expect(getArtifactPort).toHaveBeenCalledWith('same-id', 'ramble')
    expect(getActiveAction).toHaveBeenCalledWith('same-id', 'ramble')
    expect(context.routeDraftOperation).toHaveBeenCalledWith(
      'same-id',
      expect.objectContaining({ kind: 'appendAttachment' }),
      'ramble',
    )
    expect(context.saveDraftNow).not.toHaveBeenCalled()
    expect(context.applyWorkspaceMutation).not.toHaveBeenCalled()
  })
})

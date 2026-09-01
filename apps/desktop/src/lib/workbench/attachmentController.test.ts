import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  applicationCall: vi.fn(),
  beginCapture: vi.fn(),
  discardCapture: vi.fn(),
  importAttachmentPath: vi.fn(),
  leaveFullscreen: vi.fn(),
  restart: vi.fn(),
  listeners: new Map<string, (payload: unknown) => void>(),
}))

import { createAttachmentController } from './attachmentController'
import { TestApplicationTransport } from '../application/testApplicationTransport'
import { clientAttachmentFile } from '../capabilities/clientAttachmentFile'
import { defineAttachmentCandidate } from '../capabilities/capturePlugin'
import { createUnavailableWorkbenchCapabilities } from '../capabilities/unavailableCapabilities'
import type { WorkbenchCapabilities } from '../capabilities/workbenchCapabilities'

const unavailableCapabilities = createUnavailableWorkbenchCapabilities()

function availableCapabilities(): Pick<
  WorkbenchCapabilities,
  'screenCapture' | 'serverPaths' | 'windowControls'
> {
  return {
    screenCapture: {
      status: { availability: 'available', source: 'native' },
      implementation: {
        ...unavailableCapabilities.screenCapture.implementation,
        onCandidate: (handler) => {
          mocks.listeners.set('screen-capture-ready', handler as (payload: unknown) => void)
          return vi.fn()
        },
        onFinished: (handler) => {
          mocks.listeners.set('screen-capture-finished', handler as (payload: unknown) => void)
          return vi.fn()
        },
        begin: mocks.beginCapture,
      },
    },
    serverPaths: {
      status: { availability: 'available', source: 'native' },
      implementation: {
        ...unavailableCapabilities.serverPaths.implementation,
        onFileDrop: (handler) => {
          mocks.listeners.set('file-drop', handler as (payload: unknown) => void)
          return vi.fn()
        },
        importAttachmentPath: mocks.importAttachmentPath,
      },
    },
    windowControls: {
      status: { availability: 'available', source: 'native' },
      implementation: {
        ...unavailableCapabilities.windowControls.implementation,
        leaveFullscreen: mocks.leaveFullscreen,
        restart: mocks.restart,
      },
    },
  }
}

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
  const context: Parameters<typeof createAttachmentController>[0] = {
    capabilities: availableCapabilities(),
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
  }
  return {
    context,
    setBusy,
    setCaptureBusy,
  }
}

function screenCandidate() {
  return defineAttachmentCandidate({
    id: 'capture-1',
    source: 'screen-capture',
    fileName: 'shot.png',
    mediaType: 'image/png',
    byteLength: 3,
    readBytes: async () => new Uint8Array([1, 2, 3]).buffer,
    dispose: mocks.discardCapture,
  })
}

describe('attachmentController screen capture state', () => {
  beforeEach(() => {
    mocks.applicationCall.mockReset()
    mocks.beginCapture.mockReset()
    mocks.beginCapture.mockResolvedValue(undefined)
    mocks.discardCapture.mockReset()
    mocks.discardCapture.mockResolvedValue(undefined)
    mocks.importAttachmentPath.mockReset()
    mocks.leaveFullscreen.mockReset()
    mocks.leaveFullscreen.mockResolvedValue(undefined)
    mocks.restart.mockReset()
    mocks.restart.mockResolvedValue(undefined)
    mocks.listeners.clear()
    vi.stubGlobal('window', {
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('never registers a global paste listener', () => {
    const { context } = controllerContext()
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
    const { context } = controllerContext()
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
    const { context } = controllerContext()
    context.getWorkspace = () => status === null
      ? null
      : ({ request: { request_id: 'request-1', status }, attachments: [], draft: { saved_revision: 1 } }) as never
    context.getInteractionLocked = () => locked
    context.getBusy = () => busy
    const source = clientAttachmentFile({
      name: 'screen.png',
      type: 'image/png',
      size: 1,
      arrayBuffer: async () => new Uint8Array([1]).buffer,
    })

    expect(createAttachmentController(context).acceptClientFiles([source])).toBe(false)
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
    const { context } = controllerContext()
    context.getWorkspace = () => workspace as never
    mocks.applicationCall.mockImplementation(async (command: string) => {
      if (command === 'getFeedbackWorkspace') return workspace
      if (command === 'addFeedbackAttachment') return inserted
      return undefined
    })
    const source = clientAttachmentFile({
      name: 'screen.png',
      type: 'image/png',
      size: 3,
      arrayBuffer: async () => new Uint8Array([1, 2, 3]).buffer,
    })
    const controller = createAttachmentController(context)

    expect(controller.acceptClientFiles([source])).toBe(true)
    expect(controller.acceptClientFiles([source])).toBe(false)

    await vi.waitFor(() => {
      expect(mocks.applicationCall).toHaveBeenCalledWith('addFeedbackAttachment', {
        request_id: 'request-1',
        file_name: 'screen.png',
        contents: new Uint8Array([1, 2, 3]).buffer,
        expected_revision: 3,
      })
    })
    await vi.waitFor(() => expect(context.routeDraftOperation).toHaveBeenCalled())
  })

  it('allows exactly 20 MiB and rejects larger client files before reading bytes', async () => {
    const workspace = {
      request: { request_id: 'request-1', status: 'in_progress' },
      attachments: [],
      draft: { saved_revision: 3 },
    }
    const { context } = controllerContext()
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

    await controller.importClientFiles([{
      fileName: 'exact.png',
      mediaType: 'image/png',
      byteLength: 20 * 1024 * 1024,
      readBytes: exactRead,
    }])
    expect(exactRead).toHaveBeenCalledOnce()
    expect(mocks.applicationCall).toHaveBeenCalledWith(
      'addFeedbackAttachment',
      expect.objectContaining({ file_name: 'exact.png' }),
    )

    mocks.applicationCall.mockClear()
    await controller.importClientFiles([{
      fileName: 'too-large.png',
      mediaType: 'image/png',
      byteLength: 20 * 1024 * 1024 + 1,
      readBytes: tooLargeRead,
    }])
    expect(tooLargeRead).not.toHaveBeenCalled()
    expect(mocks.applicationCall).not.toHaveBeenCalledWith(
      'addFeedbackAttachment',
      expect.anything(),
    )
    expect(context.setMessage).toHaveBeenCalledWith(
      expect.stringContaining('too-large.png exceeds the 20 MiB limit'),
      'error',
    )
  })

  it('keeps capture busy until the capture finishes and blocks duplicate starts', async () => {
    const { context, setCaptureBusy } = controllerContext()
    const controller = createAttachmentController(context)

    await controller.startScreenCapture()
    await controller.startScreenCapture()

    expect(mocks.beginCapture).toHaveBeenCalledTimes(1)
    expect(setCaptureBusy).toHaveBeenCalledTimes(1)
    expect(setCaptureBusy).toHaveBeenLastCalledWith(true)
  })

  it('starts capture before Ramble when the workspace supplies the request id', async () => {
    const { context } = controllerContext()
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
    const { context } = controllerContext()
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
    const { context } = controllerContext()
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

  it('clears capture busy without changing attachment busy on cancel or pin', async () => {
    const { context, setBusy, setCaptureBusy } = controllerContext()
    const controller = createAttachmentController(context)
    const cleanup = controller.mount()
    await vi.waitFor(() => expect(mocks.listeners.has('screen-capture-finished')).toBe(true))

    mocks.listeners.get('screen-capture-finished')?.({
      candidateId: 'capture-1', outcome: 'cancelled',
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
    const file = clientAttachmentFile({
      name: 'notes.txt',
      type: 'text/plain',
      size: 4,
      arrayBuffer: async () => new Uint8Array([1, 2, 3, 4]).buffer,
    })

    await createAttachmentController(context).importClientFiles([file])

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
    const { context } = controllerContext()
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
      source: 'file-input',
      fileName: 'first.txt',
      mediaType: 'text/plain',
      byteLength: 1,
      readBytes: async () => new Uint8Array([1]).buffer,
      dispose: firstDispose,
    })
    const second = defineAttachmentCandidate({
      id: 'candidate-2',
      source: 'file-input',
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
    expect(firstDispose).toHaveBeenCalledOnce()
    expect(secondDispose).toHaveBeenCalledOnce()
  })

  it('disposes every candidate exactly once when candidate persistence fails', async () => {
    const workspace = {
      request: { request_id: 'request-1', status: 'in_progress' },
      attachments: [],
      draft: { saved_revision: 3 },
    }
    const { context } = controllerContext()
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
    expect(context.setMessage).toHaveBeenCalledWith('Error: persistence failed', 'error')
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

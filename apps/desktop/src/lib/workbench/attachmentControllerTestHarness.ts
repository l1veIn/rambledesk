import { vi } from 'vitest'

import type { WorkbenchCapabilities } from '../capabilities/workbenchCapabilities'
import { defineAttachmentCandidate } from '../capabilities/capturePlugin'
import { createUnavailableWorkbenchCapabilities } from '../capabilities/unavailableCapabilities'
import { TestApplicationTransport } from '../application/testApplicationTransport'
import { createAttachmentController } from './attachmentController'
import { createAttachmentSession } from './attachmentSession'

export const mocks = {
  applicationCall: vi.fn(),
  beginCapture: vi.fn(),
  discardCapture: vi.fn(),
  importAttachmentPath: vi.fn(),
  leaveFullscreen: vi.fn(),
  restart: vi.fn(),
  listeners: new Map<string, (payload: unknown) => void>(),
}

const unavailableCapabilities = createUnavailableWorkbenchCapabilities()

export function resetAttachmentMocks() {
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
}

export function availableCapabilities(): Pick<
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

export function controllerContext() {
  const session = createAttachmentSession()
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
    session,
    saveDraftNow: vi.fn(async () => true),
    waitForRambleMarkdown: vi.fn(async () => undefined),
    routeDraftOperation: vi.fn(async () => undefined),
    activeActionFor: () => null,
    applyWorkspaceMutation: vi.fn(),
    recordAttachmentDiagnostic: vi.fn(async () => undefined),
  }
  return { context, session }
}

export function screenCandidate() {
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

export function fileCandidate(input: Readonly<{
  fileName: string
  byteLength: number
  readBytes: () => Promise<ArrayBuffer>
  dispose?: () => Promise<void>
}>) {
  return defineAttachmentCandidate({
    id: `file-${input.fileName}`,
    source: 'file-input',
    fileName: input.fileName,
    mediaType: 'image/png',
    byteLength: input.byteLength,
    readBytes: input.readBytes,
    dispose: input.dispose ?? (async () => undefined),
  })
}

export { unavailableCapabilities }

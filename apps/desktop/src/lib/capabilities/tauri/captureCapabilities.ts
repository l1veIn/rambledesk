import type {
  ClipboardCaptureCapability,
  ScreenCaptureCapability,
} from '../workbenchCapabilities'
import { subscribeToTauriEvent } from './subscription'
import type { TauriCapabilityApi } from './tauriCapabilityApi'

export function createTauriScreenCaptureCapability(
  api: TauriCapabilityApi,
): ScreenCaptureCapability {
  return {
    onReady: (handler, onError) =>
      subscribeToTauriEvent(api, 'screen-capture-ready', handler, onError),
    onFinished: (handler, onError) =>
      subscribeToTauriEvent(api, 'screen-capture-finished', handler, onError),
    onShortcut: (handler, onError) =>
      subscribeToTauriEvent(api, 'screen-capture-shortcut', handler, onError),
    begin: () => api.invoke<void>('begin_screen_capture'),
    complete: (input) =>
      api.invoke('add_completed_screen_capture', {
        requestId: input.requestId,
        captureSessionId: input.captureSessionId,
        expectedRevision: input.expectedRevision,
      }),
    discard: (captureSessionId) =>
      api.invoke<void>('discard_screen_capture', { captureSessionId }),
  }
}

export function createTauriClipboardCaptureCapability(
  api: TauriCapabilityApi,
): ClipboardCaptureCapability {
  return {
    captureOnce: (input) =>
      api.invoke('capture_clipboard_once', {
        input: {
          request_id: input.requestId,
          ramble_context_id: input.rambleContextId,
        },
      }),
    completeImage: (input) =>
      api.invoke('add_completed_clipboard_capture', {
        requestId: input.requestId,
        captureId: input.captureId,
        rambleContextId: input.rambleContextId,
        fileName: input.fileName,
        expectedRevision: input.expectedRevision,
      }),
    discardImage: (captureId) =>
      api.invoke<void>('discard_clipboard_capture_image', { captureId }),
  }
}

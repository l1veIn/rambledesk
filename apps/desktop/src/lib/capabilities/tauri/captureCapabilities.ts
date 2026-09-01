import type {
  ClipboardCapturePlugin,
  ClipboardCaptureResult,
  ScreenCaptureFinished,
  ScreenCapturePlugin,
} from '../capturePlugin'
import { defineAttachmentCandidate } from '../capturePlugin'
import { subscribeToTauriEvent } from './subscription'
import type { TauriCapabilityApi } from './tauriCapabilityApi'

type NativeScreenCaptureReady = Readonly<{
  capture_session_id: string
  file_name: string
  byte_length: number
}>

type NativeScreenCaptureFinished = Readonly<{
  capture_session_id: string | null
  outcome: 'cancelled' | 'pinned'
}>

type NativeClipboardCaptureResult =
  | Readonly<{
      type: 'text'
      text: string
      captured_at_ms: number
      truncated: boolean
    }>
  | Readonly<{
      type: 'image'
      capture_id: string
      file_name: string
      captured_at_ms: number
      byte_length: number
    }>
  | Readonly<{ type: 'warning'; message: string }>

export function createTauriScreenCaptureCapability(
  api: TauriCapabilityApi,
): ScreenCapturePlugin {
  return {
    onCandidate: (handler, onError) =>
      subscribeToTauriEvent<NativeScreenCaptureReady>(
        api,
        'screen-capture-ready',
        (capture) => handler(screenCaptureCandidate(api, capture)),
        onError,
      ),
    onFinished: (handler, onError) =>
      subscribeToTauriEvent<NativeScreenCaptureFinished>(
        api,
        'screen-capture-finished',
        (result) => handler({
          candidateId: result.capture_session_id,
          outcome: result.outcome,
        } satisfies ScreenCaptureFinished),
        onError,
      ),
    onShortcut: (handler, onError) =>
      subscribeToTauriEvent(api, 'screen-capture-shortcut', handler, onError),
    begin: () => api.invoke<void>('begin_screen_capture'),
  }
}

export function createTauriClipboardCaptureCapability(
  api: TauriCapabilityApi,
): ClipboardCapturePlugin {
  return {
    async captureOnce() {
      const result = await api.invoke<NativeClipboardCaptureResult>('capture_clipboard_once')
      return clipboardCaptureResult(api, result)
    },
  }
}

function screenCaptureCandidate(
  api: TauriCapabilityApi,
  capture: NativeScreenCaptureReady,
) {
  return defineAttachmentCandidate({
    id: capture.capture_session_id,
    source: 'screen-capture',
    fileName: capture.file_name,
    mediaType: 'image/png',
    byteLength: capture.byte_length,
    readBytes: () => api.invoke<ArrayBuffer>('read_completed_screen_capture', {
      captureSessionId: capture.capture_session_id,
    }),
    dispose: () => api.invoke<void>('discard_screen_capture', {
      captureSessionId: capture.capture_session_id,
    }),
  })
}

function clipboardCaptureResult(
  api: TauriCapabilityApi,
  result: NativeClipboardCaptureResult,
): ClipboardCaptureResult {
  if (result.type === 'warning') throw new Error(result.message)
  if (result.type === 'text') {
    return {
      kind: 'text',
      text: result.text,
      capturedAtMs: result.captured_at_ms,
      truncated: result.truncated,
    }
  }
  return {
    kind: 'attachment',
    capturedAtMs: result.captured_at_ms,
    candidate: defineAttachmentCandidate({
      id: result.capture_id,
      source: 'clipboard-image',
      fileName: result.file_name,
      mediaType: 'image/png',
      byteLength: result.byte_length,
      readBytes: () => api.invoke<ArrayBuffer>('read_clipboard_capture_image', {
        captureId: result.capture_id,
      }),
      dispose: () => api.invoke<void>('discard_clipboard_capture_image', {
        captureId: result.capture_id,
      }),
    }),
  }
}

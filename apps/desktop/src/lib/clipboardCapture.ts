export type ClipboardCaptureEvent =
  | {
      type: 'text'
      request_id: string
      ramble_session_id: string
      text: string
      captured_at_ms: number
      truncated: boolean
    }
  | {
      type: 'image'
      request_id: string
      ramble_session_id: string
      capture_id: string
      file_name: string
      captured_at_ms: number
    }
  | {
      type: 'warning'
      request_id: string
      ramble_session_id: string
      message: string
    }

export function eventBelongsToRamble(
  event: ClipboardCaptureEvent,
  requestId: string,
  rambleSessionId: string,
): boolean {
  return (
    event.request_id === requestId &&
    event.ramble_session_id === rambleSessionId
  )
}

export function clipboardCaptureLabel(
  capturedAtMs: number,
  truncated = false,
  locale: Locale = 'zh-CN',
): string {
  const time = new Date(capturedAtMs).toLocaleTimeString(locale, {
    hour12: false,
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
  if (locale === 'en') {
    return `Clipboard import · ${time}${truncated ? ' · Content truncated' : ''}`
  }
  return `剪贴板捕获 · ${time}${truncated ? ' · 内容过长，已截断' : ''}`
}
import type { Locale } from './preferences'

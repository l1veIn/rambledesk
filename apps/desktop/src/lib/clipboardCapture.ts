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

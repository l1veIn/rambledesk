import { isSafeHttpUrl } from './linkify'

export async function openExternalUrl(href: string) {
  if (!isSafeHttpUrl(href)) return
  if ('__TAURI_INTERNALS__' in window) {
    const { openUrl } = await import('@tauri-apps/plugin-opener')
    await openUrl(href)
    return
  }
  window.open(href, '_blank', 'noopener,noreferrer')
}

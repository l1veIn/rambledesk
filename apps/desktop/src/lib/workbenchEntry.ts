export type WorkbenchEntry =
  | 'browser'
  | 'preview'
  | 'desktop'
  | 'capture'
  | 'scroll-capture'
  | 'pinned-capture'
  | 'ramble-console'

export type WorkbenchEntryInput = Readonly<{
  isTauri: boolean
  previewMode: boolean
  pathname: string
  hash: string
}>

/**
 * Select the application root before interpreting Tauri-only routes.
 * Browser history paths and hashes must never select a native capability root.
 */
export function selectWorkbenchEntry(input: WorkbenchEntryInput): WorkbenchEntry {
  if (!input.isTauri) return input.previewMode ? 'preview' : 'browser'
  if (input.hash === '#capture') return 'capture'
  if (input.hash === '#capture-scroll') return 'scroll-capture'
  if (input.hash.startsWith('#capture-pin=')) return 'pinned-capture'
  if (input.hash === '#ramble-console' || input.pathname.endsWith('/ramble-console')) {
    return 'ramble-console'
  }
  return 'desktop'
}

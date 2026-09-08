/**
 * Viewport classes for the workbench shell.
 *
 * Phones turn both rails into overlay drawers; tablet and desktop keep the
 * resizable columns. The numeric bounds live here so the shell and the tests
 * agree on a single definition.
 */
export type ShellMode = 'desktop' | 'tablet' | 'phone'

export const PHONE_MAX_WIDTH = 767.98
export const TABLET_MIN_WIDTH = 768
export const TABLET_MAX_WIDTH = 1023.98

export const PHONE_QUERY = `(max-width: ${PHONE_MAX_WIDTH}px)`
export const TABLET_QUERY = `(min-width: ${TABLET_MIN_WIDTH}px) and (max-width: ${TABLET_MAX_WIDTH}px)`

export function shellModeFor(phoneMatches: boolean, tabletMatches: boolean): ShellMode {
  if (phoneMatches) return 'phone'
  if (tabletMatches) return 'tablet'
  return 'desktop'
}

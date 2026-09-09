/**
 * Viewport classes for the workbench shell.
 *
 * Phones turn both rails into overlay drawers; tablet and desktop keep the
 * resizable columns. The breakpoints themselves live in `$lib/mediaQuery` so the
 * shell and the agent composer share one definition.
 */
export type ShellMode = 'desktop' | 'tablet' | 'phone'

export {
  PHONE_MAX_WIDTH,
  PHONE_QUERY,
  TABLET_MAX_WIDTH,
  TABLET_MIN_WIDTH,
  TABLET_QUERY,
} from '../mediaQuery'

export function shellModeFor(phoneMatches: boolean, tabletMatches: boolean): ShellMode {
  if (phoneMatches) return 'phone'
  if (tabletMatches) return 'tablet'
  return 'desktop'
}

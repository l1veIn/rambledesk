/** Uniform tab widths for the titlebar strip. */
export const TAB_MIN_WIDTH = 112
export const TAB_MAX_WIDTH = 192

export type TabStripLayout = Readonly<{
  /** Every tab gets this width; they never shrink individually. */
  tabWidth: number
  /** True once the minimum width no longer fits and the strip has to scroll. */
  overflowing: boolean
}>

/**
 * Tabs share the available width evenly, clamped to [MIN, MAX]. Once every tab
 * would fall below the minimum, they stay at the minimum and the strip scrolls.
 * The same rule applies on every viewport, so desktop and phones behave alike.
 */
export function tabStripLayout(available: number, count: number): TabStripLayout {
  if (count <= 0) return { tabWidth: TAB_MAX_WIDTH, overflowing: false }
  if (!Number.isFinite(available) || available <= 0) {
    return { tabWidth: TAB_MAX_WIDTH, overflowing: false }
  }
  const fitted = Math.floor(available / count)
  if (fitted < TAB_MIN_WIDTH) return { tabWidth: TAB_MIN_WIDTH, overflowing: true }
  return { tabWidth: Math.min(TAB_MAX_WIDTH, fitted), overflowing: false }
}

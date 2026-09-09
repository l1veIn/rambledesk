import { readable, type Readable } from 'svelte/store'

/**
 * Viewport breakpoints. They live here so the shell, the agent composer, and the
 * tests all agree on one definition.
 */
export const PHONE_MAX_WIDTH = 767.98
export const TABLET_MIN_WIDTH = 768
export const TABLET_MAX_WIDTH = 1023.98

export const PHONE_QUERY = `(max-width: ${PHONE_MAX_WIDTH}px)`
export const TABLET_QUERY = `(min-width: ${TABLET_MIN_WIDTH}px) and (max-width: ${TABLET_MAX_WIDTH}px)`

/** Follows a CSS media query; stays false without a browser (SSR, node tests). */
export function mediaQuery(query: string): Readable<boolean> {
  return readable(false, (set) => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return
    const list = window.matchMedia(query)
    const update = () => set(list.matches)
    update()
    list.addEventListener('change', update)
    return () => list.removeEventListener('change', update)
  })
}

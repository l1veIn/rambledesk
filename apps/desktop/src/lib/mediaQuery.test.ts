// @vitest-environment jsdom
import { get } from 'svelte/store'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { mediaQuery, PHONE_QUERY } from './mediaQuery'

type Listener = () => void

function fakeMatchMedia(matches: boolean) {
  const listeners = new Set<Listener>()
  const list = {
    matches,
    media: PHONE_QUERY,
    addEventListener: (_type: string, listener: Listener) => listeners.add(listener),
    removeEventListener: (_type: string, listener: Listener) => listeners.delete(listener),
  }
  window.matchMedia = vi.fn(() => list) as never
  return {
    listeners,
    emit(next: boolean) {
      list.matches = next
      for (const listener of listeners) listener()
    },
  }
}

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('media query store', () => {
  it('starts from the current match and follows changes', () => {
    const media = fakeMatchMedia(true)
    const values: boolean[] = []
    const unsubscribe = mediaQuery(PHONE_QUERY).subscribe((value) => values.push(value))

    expect(values).toEqual([true])
    media.emit(false)
    expect(values).toEqual([true, false])
    unsubscribe()
    expect(media.listeners.size).toBe(0)
  })

  it('stays false without matchMedia', () => {
    vi.stubGlobal('window', {})
    expect(get(mediaQuery(PHONE_QUERY))).toBe(false)
  })
})

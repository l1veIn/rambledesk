import { describe, expect, it } from 'vitest'
import { PHONE_QUERY, shellModeFor, TABLET_QUERY } from './shellMode'

describe('shell mode', () => {
  it('maps viewport queries to the shell presentation', () => {
    expect(shellModeFor(true, false)).toBe('phone')
    expect(shellModeFor(false, true)).toBe('tablet')
    expect(shellModeFor(false, false)).toBe('desktop')
    // A phone query wins when both match, which keeps the drawer layout conservative.
    expect(shellModeFor(true, true)).toBe('phone')
  })

  it('keeps the phone and tablet ranges disjoint', () => {
    const phoneMax = Number(PHONE_QUERY.match(/max-width: ([\d.]+)px/)![1])
    const tabletMin = Number(TABLET_QUERY.match(/min-width: ([\d.]+)px/)![1])
    expect(tabletMin).toBeGreaterThan(phoneMax)
  })
})

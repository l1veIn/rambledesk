import { describe, expect, it, vi } from 'vitest'

import {
  DESKTOP_NAVIGATION_POLL_INTERVAL_MS,
  ensureDesktopNavigationPolling,
} from './navigationPolling'

describe('ensureDesktopNavigationPolling', () => {
  it('installs one desktop retry timer independently of initial navigation success', () => {
    const refresh = vi.fn()
    let scheduledCallback: (() => void) | undefined
    const schedule = vi.fn((callback: () => void, delayMs: number) => {
      scheduledCallback = callback
      expect(delayMs).toBe(DESKTOP_NAVIGATION_POLL_INTERVAL_MS)
      return 17
    })

    const timer = ensureDesktopNavigationPolling(true, undefined, schedule, refresh)
    const sameTimer = ensureDesktopNavigationPolling(true, timer, schedule, refresh)
    scheduledCallback?.()

    expect(timer).toBe(17)
    expect(sameTimer).toBe(17)
    expect(schedule).toHaveBeenCalledTimes(1)
    expect(refresh).toHaveBeenCalledTimes(1)
  })

  it('does not poll browser or preview compositions', () => {
    const schedule = vi.fn(() => 17)

    expect(ensureDesktopNavigationPolling(false, undefined, schedule, vi.fn())).toBeUndefined()
    expect(schedule).not.toHaveBeenCalled()
  })
})

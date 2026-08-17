import { describe, expect, it } from 'vitest'

import { detectDesktopPlatform } from './platform'

describe('detectDesktopPlatform', () => {
  it('recognizes current and legacy macOS identities', () => {
    expect(detectDesktopPlatform('MacIntel', '')).toBe('macOS')
    expect(detectDesktopPlatform('', 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)')).toBe('macOS')
  })

  it('keeps Windows updater support and falls back to Linux', () => {
    expect(detectDesktopPlatform('Win32', '')).toBe('Windows')
    expect(detectDesktopPlatform('Linux x86_64', 'Mozilla/5.0 (X11; Linux x86_64)')).toBe('Linux')
  })
})

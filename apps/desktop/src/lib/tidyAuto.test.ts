import { describe, expect, it } from 'vitest'

import {
  MAX_TIDY_AUTO_THRESHOLD,
  normalizeTidyAutoThreshold,
  shouldAutoTidy,
} from './tidyAuto'

describe('Tidy automatic threshold', () => {
  it('treats zero as disabled', () => {
    expect(shouldAutoTidy(100, 0)).toBe(false)
  })

  it('runs when the pending count reaches the threshold', () => {
    expect(shouldAutoTidy(2, 3)).toBe(false)
    expect(shouldAutoTidy(3, 3)).toBe(true)
    expect(shouldAutoTidy(4, 3)).toBe(true)
  })

  it('normalizes persisted and user-entered values', () => {
    expect(normalizeTidyAutoThreshold(Number.NaN)).toBe(0)
    expect(normalizeTidyAutoThreshold(-2)).toBe(0)
    expect(normalizeTidyAutoThreshold(3.6)).toBe(4)
    expect(normalizeTidyAutoThreshold(2000)).toBe(MAX_TIDY_AUTO_THRESHOLD)
  })
})

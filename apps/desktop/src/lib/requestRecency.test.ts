import { describe, expect, it } from 'vitest'

import { isWithinLast24Hours } from './requestRecency'

describe('isWithinLast24Hours', () => {
  const now = Date.parse('2026-08-18T12:00:00.000Z')

  it('keeps updates from the last 24 hours', () => {
    expect(isWithinLast24Hours('2026-08-17T12:00:00.000Z', now)).toBe(true)
    expect(isWithinLast24Hours('2026-08-18T11:59:00.000Z', now)).toBe(true)
  })

  it('drops updates older than 24 hours and invalid timestamps', () => {
    expect(isWithinLast24Hours('2026-08-17T11:59:59.000Z', now)).toBe(false)
    expect(isWithinLast24Hours('not-a-date', now)).toBe(false)
  })
})

import { describe, expect, it } from 'vitest'
import { contextUsageDisplay } from './contextUsage'

describe('Agent reported context usage', () => {
  it('shows actual counts and derives only their percentage', () => {
    expect(contextUsageDisplay({ used: 64000, size: 200000 })).toEqual({ percent: 32, used: '64,000', size: '200,000' })
    expect(contextUsageDisplay({ used: 0, size: 1000 })?.percent).toBe(0)
    expect(contextUsageDisplay({ used: 110, size: 100 })?.percent).toBe(110)
  })
  it('hides unavailable or invalid telemetry without estimating it', () => {
    for (const usage of [undefined, null, { used: 3, size: 0 }, { used: -1, size: 100 }, { used: 1.5, size: 100 }, { used: Number.MAX_VALUE, size: 100 }]) {
      expect(contextUsageDisplay(usage)).toBeNull()
    }
  })
})

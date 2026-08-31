import { describe, expect, it } from 'vitest'

import { timelineTurnStartsOpen } from './timelinePresentation'

describe('timelineTurnStartsOpen', () => {
  it('keeps a completed Turn open until another Turn starts', () => {
    expect(timelineTurnStartsOpen(0, 1)).toBe(true)
  })

  it('folds only earlier Turns after the next round exists', () => {
    expect(timelineTurnStartsOpen(0, 2)).toBe(false)
    expect(timelineTurnStartsOpen(1, 2)).toBe(true)
  })
})

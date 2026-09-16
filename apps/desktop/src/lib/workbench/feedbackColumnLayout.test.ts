import { describe, expect, it } from 'vitest'

import {
  FEEDBACK_COLUMN,
  FEEDBACK_PANE_MIN_PERCENT,
  feedbackColumnLayout,
  feedbackWidthFromLayout,
} from './feedbackColumnLayout'

describe('workspace column width policy', () => {
  function feedbackPx(containerPx: number, savedPx?: number | null): number {
    return (feedbackColumnLayout({ containerPx, savedPx }).feedback / 100) * containerPx
  }

  it('gives the feedback column its preferred width while the brief keeps its floor', () => {
    expect(feedbackPx(1600)).toBeCloseTo(700, 5)
    expect(feedbackPx(1440)).toBeCloseTo(700, 5)
    // 1600 - 700 leaves the brief more than its 420 floor.
    expect(1600 - feedbackPx(1600)).toBeGreaterThanOrEqual(420)
  })

  it('never takes the brief below its floor when the container is smaller', () => {
    // 1100 - 420 leaves 680 for feedback: the 700 target gives way, the 560 floor stays.
    expect(feedbackPx(1100)).toBeCloseTo(680, 5)
    // 900 cannot hold both floors, so the brief's floor wins and feedback takes the rest.
    // (Below the wide breakpoint the columns stack instead, so this stays a corner case.)
    expect(feedbackPx(900)).toBeCloseTo(480, 5)
    expect(900 - feedbackPx(900)).toBeCloseTo(420, 5)
  })

  it('falls back to a share of tiny containers instead of overflowing', () => {
    const tiny = feedbackColumnLayout({ containerPx: 400 })
    expect(tiny.feedback).toBeCloseTo(FEEDBACK_PANE_MIN_PERCENT, 5)
    expect(tiny.brief + tiny.feedback).toBe(100)
  })

  it('clamps a dragged width to the policy range', () => {
    expect(feedbackPx(2000, 200)).toBeCloseTo(FEEDBACK_COLUMN.min, 5)
    expect(feedbackPx(2000, 5000)).toBeCloseTo(FEEDBACK_COLUMN.max, 5)
    expect(feedbackPx(2000, 820)).toBeCloseTo(820, 5)
  })

  it('keeps a usable layout before the container is measured', () => {
    const unmeasured = feedbackColumnLayout({ containerPx: 0 })
    expect(unmeasured.feedback).toBe(30)
    expect(unmeasured.brief + unmeasured.feedback).toBe(100)
  })

  it('reads a dragged layout back as pixels and rejects unusable ones', () => {
    expect(feedbackWidthFromLayout([31.25, 68.75], 1600)).toBe(1100)
    expect(feedbackWidthFromLayout([100], 1600)).toBeNull()
    expect(feedbackWidthFromLayout([40, 0], 1600)).toBeNull()
    expect(feedbackWidthFromLayout([40, Number.NaN], 1600)).toBeNull()
    expect(feedbackWidthFromLayout([31.25, 68.75], 0)).toBeNull()
  })
})

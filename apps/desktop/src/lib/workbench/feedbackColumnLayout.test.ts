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

  it('gives the workbench most of the workspace and feedback a compact preferred width', () => {
    expect(feedbackPx(1200)).toBeCloseTo(400, 5)
    expect(feedbackPx(1440)).toBeCloseTo(400, 5)
    expect(feedbackPx(960)).toBeCloseTo(320, 5)
    expect(feedbackColumnLayout({ containerPx: 1200 }).brief).toBeCloseTo(200 / 3, 5)
    expect(1200 - feedbackPx(1200)).toBeGreaterThanOrEqual(520)
  })

  it('never takes the workbench below its floor when the container is smaller', () => {
    // Feedback stops narrowing at 320 when both columns can still keep their floors.
    expect(feedbackPx(900)).toBeCloseTo(320, 5)
    // 800 cannot hold both floors, so the workbench's floor wins and feedback takes the rest.
    // (Below the wide breakpoint the columns stack instead, so this stays a corner case.)
    expect(feedbackPx(800)).toBeCloseTo(280, 5)
    expect(800 - feedbackPx(800)).toBeCloseTo(520, 5)
  })

  it('falls back to a share of tiny containers instead of overflowing', () => {
    const tiny = feedbackColumnLayout({ containerPx: 400 })
    expect(tiny.feedback).toBeCloseTo(FEEDBACK_PANE_MIN_PERCENT, 5)
    expect(tiny.brief + tiny.feedback).toBe(100)
  })

  it('clamps a dragged width to the policy range', () => {
    expect(feedbackPx(2000, 200)).toBeCloseTo(FEEDBACK_COLUMN.min, 5)
    expect(feedbackPx(2000, 5000)).toBeCloseTo(FEEDBACK_COLUMN.max, 5)
    expect(feedbackPx(2000, 480)).toBeCloseTo(480, 5)
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

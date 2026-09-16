/**
 * Width policy for the Ramble workspace columns.
 *
 * The feedback column must stay wide enough for the editor and its tool row, so
 * the split is expressed in pixels and only converted to the percentages
 * PaneForge stores. A saved width is re-clamped on every window size, and the
 * task brief keeps its own floor.
 */
export const FEEDBACK_COLUMN = Object.freeze({
  /** Preferred width once the window has room for it. */
  target: 700,
  min: 560,
  max: 900,
  /** Below this the task brief can no longer show its two-column grid. */
  briefMin: 420,
})

export const TASK_BRIEF_PANE = Object.freeze({
  defaultPercent: 30,
  minPercent: 8,
  maxPercent: 40,
})

export const FEEDBACK_PANE_MIN_PERCENT = 45

export type ColumnLayout = Readonly<{ brief: number; feedback: number }>

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max)
}

/**
 * Converts the pixel policy into the two pane percentages.
 *
 * `savedPx` is the reader's last drag; it is clamped to the policy and to
 * whatever the container can spare after the brief's floor.
 */
export function feedbackColumnLayout(input: Readonly<{
  containerPx: number
  savedPx?: number | null
}>): ColumnLayout {
  const container = input.containerPx
  if (!Number.isFinite(container) || container <= 0) {
    return { brief: 100 - TASK_BRIEF_PANE.defaultPercent, feedback: TASK_BRIEF_PANE.defaultPercent }
  }

  const wanted = clamp(input.savedPx ?? FEEDBACK_COLUMN.target, FEEDBACK_COLUMN.min, FEEDBACK_COLUMN.max)
  // Never widen past the container, and keep the brief's floor whenever the
  // container can still afford it.
  const floor = Math.min(FEEDBACK_COLUMN.min, container * (FEEDBACK_PANE_MIN_PERCENT / 100))
  const ceiling = Math.max(floor, container - FEEDBACK_COLUMN.briefMin)
  const feedbackPx = clamp(wanted, floor, ceiling)
  const feedback = (feedbackPx / container) * 100
  return { brief: 100 - feedback, feedback }
}

/** The dragged layout as a durable pixel width, or null when it is unusable. */
export function feedbackWidthFromLayout(layout: readonly number[], containerPx: number): number | null {
  if (layout.length < 2 || !Number.isFinite(containerPx) || containerPx <= 0) return null
  const percent = layout[1]
  if (typeof percent !== 'number' || !Number.isFinite(percent) || percent <= 0) return null
  return Math.round((percent / 100) * containerPx)
}

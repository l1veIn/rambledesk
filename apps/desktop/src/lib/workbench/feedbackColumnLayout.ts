/**
 * Width policy for the Ramble workspace columns.
 *
 * The workbench gets most of the workspace, while the feedback column keeps
 * enough room for its editor and shared input console. The split is expressed
 * in pixels and only converted to the percentages
 * PaneForge stores. A saved width is re-clamped on every window size, and the
 * workbench keeps its own floor.
 */
export const FEEDBACK_COLUMN = Object.freeze({
  /** Preferred width once the window has room for it. */
  target: 400,
  min: 320,
  max: 640,
  /** Keep the main workbench wide enough to read and review its content. */
  briefMin: 520,
})

export const TASK_BRIEF_PANE = Object.freeze({
  defaultPercent: 30,
  minPercent: 8,
  maxPercent: 40,
})

export const FEEDBACK_PANE_MIN_PERCENT = 20

export type ColumnLayout = Readonly<{ brief: number; feedback: number }>

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max)
}

/**
 * Converts the pixel policy into the two pane percentages.
 *
 * `savedPx` is the reader's last drag; it is clamped to the policy and to
 * whatever the container can spare after the workbench's floor.
 */
export function feedbackColumnLayout(input: Readonly<{
  containerPx: number
  savedPx?: number | null
}>): ColumnLayout {
  const container = input.containerPx
  if (!Number.isFinite(container) || container <= 0) {
    return { brief: 100 - TASK_BRIEF_PANE.defaultPercent, feedback: TASK_BRIEF_PANE.defaultPercent }
  }

  const preferred = Math.min(FEEDBACK_COLUMN.target, container / 3)
  const wanted = clamp(input.savedPx ?? preferred, FEEDBACK_COLUMN.min, FEEDBACK_COLUMN.max)
  // Never widen past the container, and keep the workbench's floor whenever the
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

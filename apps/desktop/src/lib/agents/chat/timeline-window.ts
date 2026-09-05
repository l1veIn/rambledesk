import type { SessionActivity } from '../managedSessionUi'

export const ACTIVITY_WINDOW_SIZE = 60

/** A render window never drops the already-loaded beginning of its first turn. */
function turnBoundary(activities: readonly SessionActivity[], start: number): number {
  const turn = activities[start]?.turn_id
  if (!turn) return start
  while (start > 0 && activities[start - 1].turn_id === turn) start -= 1
  return start
}

/** Keep rendering bounded initially, and preserve the first visible row while reviewing history. */
export class TimelineWindow {
  #sessionId = ''
  #firstId: string | null = null
  #count = ACTIVITY_WINDOW_SIZE

  read(sessionId: string, activities: readonly SessionActivity[], followLatest: boolean): readonly SessionActivity[] {
    if (this.#sessionId !== sessionId) {
      this.#sessionId = sessionId
      this.#count = ACTIVITY_WINDOW_SIZE
      this.#firstId = null
    }
    const previous = activities.findIndex((activity) => activity.id === this.#firstId)
    const start = turnBoundary(activities, followLatest || previous < 0 ? Math.max(0, activities.length - this.#count) : previous)
    this.#firstId = activities[start]?.id ?? null
    return activities.slice(start)
  }

  revealOlder(activities: readonly SessionActivity[]): void {
    const previous = activities.findIndex((activity) => activity.id === this.#firstId)
    const start = turnBoundary(activities, Math.max(0, (previous < 0 ? activities.length - this.#count : previous) - ACTIVITY_WINDOW_SIZE))
    this.#firstId = activities[start]?.id ?? null
    this.#count = activities.length - start
  }
}

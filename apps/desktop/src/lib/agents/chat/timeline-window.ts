import type { SessionActivity } from '../managedSessionUi'
import { HISTORY_TURN_COUNT } from '../activityHistory'

/** Count conversation turns, not the tool/thinking events hidden inside them. */
function turnStarts(activities: readonly SessionActivity[]): number[] {
  const starts: number[] = []
  for (let index = 0; index < activities.length; index += 1) {
    const row = activities[index]
    const previous = activities[index - 1]
    if (!previous || row.kind === 'user_message'
      || (previous.kind !== 'user_message' && (!row.turn_id || row.turn_id !== previous.turn_id))) starts.push(index)
  }
  return starts
}

/** A render window never drops the already-loaded beginning of its first turn. */
function turnBoundary(activities: readonly SessionActivity[], start: number): number {
  const turn = activities[start]?.turn_id
  if (!turn) return start
  while (start > 0 && activities[start - 1].turn_id === turn) start -= 1
  if (start > 0 && activities[start - 1].kind === 'user_message') start -= 1
  return start
}

/** Keep rendering bounded initially, and preserve the first visible row while reviewing history. */
export class TimelineWindow {
  #sessionId = ''
  #firstId: string | null = null
  #count = HISTORY_TURN_COUNT

  read(sessionId: string, activities: readonly SessionActivity[], followLatest: boolean): readonly SessionActivity[] {
    if (this.#sessionId !== sessionId) {
      this.#sessionId = sessionId
      this.#count = HISTORY_TURN_COUNT
      this.#firstId = null
    }
    const previous = activities.findIndex((activity) => activity.id === this.#firstId)
    const starts = turnStarts(activities)
    const start = turnBoundary(activities, followLatest || previous < 0 ? starts[Math.max(0, starts.length - this.#count)] ?? 0 : previous)
    this.#firstId = activities[start]?.id ?? null
    return activities.slice(start)
  }

  revealOlder(activities: readonly SessionActivity[]): void {
    const previous = activities.findIndex((activity) => activity.id === this.#firstId)
    const starts = turnStarts(activities)
    const at = previous < 0 ? Math.max(0, starts.length - this.#count) : Math.max(0, starts.findLastIndex(start => start <= previous))
    const start = starts[Math.max(0, at - HISTORY_TURN_COUNT)] ?? 0
    this.#firstId = activities[start]?.id ?? null
    this.#count = starts.length - starts.indexOf(start)
  }
}

/** A near-top crossing only: initial layout and short lists never drain history. */
export function crossedHistoryThreshold(previous: number | null, next: number): boolean {
  return previous !== null && previous >= 240 && next < 240
}

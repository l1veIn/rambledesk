import type { ManagedSessionSnapshot, SessionActivity } from '$lib/generated/feedback'

export const HISTORY_TURN_COUNT = 20
export const HISTORY_ACTIVITY_LIMIT = 1000

export type CompletedHistoryRange = Readonly<{ turnId: string; first: number; before: number }>

/** Re-read only a just-completed turn's already-visible rows outside the live tail. */
export function completedHistoryRanges(previous: ManagedSessionSnapshot | null, next: ManagedSessionSnapshot): CompletedHistoryRange[] {
  if (!previous || next.deleting || !next.activities[0]) return []
  const completed = new Set(next.activities.filter(row => row.turn_id
    && ((row.kind === 'status' && row.text.startsWith('Turn finished: '))
      || (row.kind === 'error' && row.text === 'Turn interrupted before completion.'))
    && !previous.activities.some(before => before.id === row.id)).map(row => row.turn_id!))
  const previousActive = previous.runtime.activity !== 'idle' ? previous.activities.findLast(row => row.turn_id)?.turn_id : null
  const nextActive = next.runtime.activity !== 'idle' ? next.activities.findLast(row => row.turn_id)?.turn_id : null
  if (previousActive && previousActive !== nextActive) completed.add(previousActive)
  const ranges = new Map<string, CompletedHistoryRange>()
  for (const row of previous.activities) {
    if (!row.turn_id || !completed.has(row.turn_id) || row.sequence >= next.activities[0].sequence) continue
    const range = ranges.get(row.turn_id)
    ranges.set(row.turn_id, { turnId: row.turn_id, first: range?.first ?? row.sequence, before: row.sequence + 1 })
  }
  return [...ranges.values()]
}

/** Fresh live rows replace historical versions; page overlap never duplicates a card. */
export function mergeActivityWindows(older: readonly SessionActivity[], current: readonly SessionActivity[]) {
  const result: SessionActivity[] = []
  let previous = 0
  let fresh = 0
  while (previous < older.length && fresh < current.length) {
    if (older[previous].sequence < current[fresh].sequence) result.push(older[previous++])
    else {
      if (older[previous].sequence === current[fresh].sequence) previous += 1
      result.push(current[fresh++])
    }
  }
  return result.concat(older.slice(previous), current.slice(fresh))
}

/** Transport snapshots decode new objects; keep unchanged rows stable for turn rendering. */
export function retainActivityIdentity(previous: readonly SessionActivity[], fresh: readonly SessionActivity[]) {
  const byId = new Map(previous.map(row => [row.id, row]))
  return fresh.map(row => {
    const before = byId.get(row.id)
    return before && before.sequence === row.sequence && before.session_id === row.session_id
      && before.turn_id === row.turn_id && before.kind === row.kind && before.text === row.text
      && before.tool_call_id === row.tool_call_id && before.created_at === row.created_at
      && (before.content === row.content || JSON.stringify(before.content) === JSON.stringify(row.content)) ? before : row
  })
}

export function validateActivityPage(activities: readonly SessionActivity[], sessionId: string, before: number) {
  if (activities.some((row, index) => row.session_id !== sessionId || row.sequence < 1 || row.sequence >= before || (index > 0 && row.sequence <= activities[index - 1].sequence))) {
    throw new Error('The agent history returned an invalid session or cursor.')
  }
}

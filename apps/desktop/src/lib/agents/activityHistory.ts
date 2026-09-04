import type { SessionActivity } from '$lib/generated/feedback'

/** Fresh live rows replace historical versions; page overlap never duplicates a card. */
export function mergeActivityWindows(older: readonly SessionActivity[], current: readonly SessionActivity[]) {
  const byId = new Map(older.map(activity => [activity.id, activity]))
  for (const activity of current) byId.set(activity.id, activity)
  return [...byId.values()].sort((left, right) => left.sequence - right.sequence)
}

export function validateActivityPage(activities: readonly SessionActivity[], sessionId: string, before: number) {
  if (activities.some((row, index) => row.session_id !== sessionId || row.sequence < 1 || row.sequence >= before || (index > 0 && row.sequence <= activities[index - 1].sequence))) {
    throw new Error('The agent history returned an invalid session or cursor.')
  }
}

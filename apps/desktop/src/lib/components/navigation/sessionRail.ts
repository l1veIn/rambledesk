import type { HostSessionSummary } from '$lib/feedback'

export function hostSessionKey(session: HostSessionSummary): string {
  return `${session.host_id}\u0000${session.host_session_id}`
}

export function orderSessionRailSessions(
  sessions: readonly HostSessionSummary[],
): HostSessionSummary[] {
  return [...sessions].sort((left, right) => {
    return (
      compareNullableIsoDesc(left.host_pinned_at, right.host_pinned_at) ||
      compareNullableIsoDesc(left.pinned_at, right.pinned_at) ||
      right.updated_at.localeCompare(left.updated_at) ||
      left.host_id.localeCompare(right.host_id) ||
      left.host_session_id.localeCompare(right.host_session_id)
    )
  })
}

function compareNullableIsoDesc(
  left: string | null | undefined,
  right: string | null | undefined,
): number {
  if (left === right) return 0
  if (!left) return 1
  if (!right) return -1
  return right.localeCompare(left)
}

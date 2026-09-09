import type { FeedbackRequestSummary, ListFeedbackRequestsInput } from '../../feedback'
import { requestFilterStatuses } from '../../domain/requestFilters'
import type { NavigationState } from './navigationTypes'

/** Snapshot list requests for the current scope, search and filters. */
export function requestListInput(
  state: NavigationState,
  cursor: string | null = null,
): ListFeedbackRequestsInput {
  return {
    host_id: state.selectedHostId,
    host_session_id: state.selectedHostSessionId,
    status: requestFilterStatuses(state.requestFilters.status),
    archived: null,
    search: state.requestSearch.trim() || null,
    limit: 100,
    cursor,
  }
}

/** Identity of the query whose rows are currently displayed. */
export function requestQueryKey(state: NavigationState): string {
  return JSON.stringify([
    state.selectedHostId,
    state.selectedHostSessionId,
    state.requestSearch.trim(),
    state.requestFilters.status,
    state.requestFilters.timeRange,
  ])
}

export function requestMatchesSearch(request: FeedbackRequestSummary, search: string): boolean {
  const normalized = search.trim().toLowerCase()
  if (!normalized) return true
  return [
    request.title,
    request.what_happened,
    request.source_hint,
    request.request_id,
    request.host_id,
    request.host_session_id,
  ].some((value) => (value ?? '').toLowerCase().includes(normalized))
}

export function now(): number {
  return typeof performance === 'undefined' ? Date.now() : performance.now()
}

export async function waitForMinimumDuration(startedAt: number, minimumMs: number): Promise<void> {
  const remainingMs = minimumMs - (now() - startedAt)
  if (remainingMs <= 0) return
  await new Promise((resolve) => setTimeout(resolve, remainingMs))
}

import type { FeedbackStatus, ListFeedbackRequestsOutput } from '../feedback'

export const REQUEST_STATUS_FILTERS = ['all', 'pending', 'waiting', 'in_progress', 'completed', 'cancelled'] as const
export const REQUEST_TIME_RANGES = ['all', '24h', '7d', '30d'] as const
export type RequestStatusFilter = typeof REQUEST_STATUS_FILTERS[number]
export type RequestTimeRange = typeof REQUEST_TIME_RANGES[number]
export type RequestFilters = Readonly<{
  status: RequestStatusFilter
  timeRange: RequestTimeRange
}>

export const DEFAULT_REQUEST_FILTERS: RequestFilters = { status: 'all', timeRange: 'all' }

export function requestFilterCount(filters: RequestFilters) {
  return Number(filters.status !== 'all') + Number(filters.timeRange !== 'all')
}

export function requestFilterStatuses(status: RequestStatusFilter): FeedbackStatus[] {
  if (status === 'all') return ['waiting', 'in_progress', 'completed', 'cancelled']
  if (status === 'pending') return ['waiting', 'in_progress']
  return [status]
}

export function filterRequestPage(
  page: ListFeedbackRequestsOutput,
  timeRange: RequestTimeRange,
  now = Date.now(),
): ListFeedbackRequestsOutput {
  if (timeRange === 'all') return page
  const days = timeRange === '24h' ? 1 : timeRange === '7d' ? 7 : 30
  const cutoff = now - days * 24 * 60 * 60 * 1000
  const requests = page.requests.filter((request) => Date.parse(request.updated_at) >= cutoff)
  return {
    requests,
    // The API sorts by updated_at descending. Once a page crosses the cutoff,
    // later pages cannot contain matches; otherwise keep pagination available.
    next_cursor: requests.length === page.requests.length ? page.next_cursor : null,
  }
}

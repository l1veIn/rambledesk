import type { FeedbackRequestSummary, HostSessionSummary } from '../../feedback'
import { DEFAULT_REQUEST_FILTERS, type RequestFilters } from '../requestFilters'
import type { HostProfile } from '../types'

/** Manual refreshes keep a floor so the spinner never flashes. */
export const MANUAL_PAGE_REFRESH_MIN_MS = 300

export type NavigationState = {
  pendingRequests: FeedbackRequestSummary[]
  requests: FeedbackRequestSummary[]
  hostSessions: HostSessionSummary[]
  hostSessionFactsStatus: 'pending' | 'ready' | 'failed'
  hostSessionFactsRevision: number
  hostProfiles: Record<string, HostProfile>
  selectedHostId: string | null
  selectedHostSessionId: string | null
  requestSearch: string
  requestFilters: RequestFilters
  nextRequestCursor: string | null
  loadingNavigation: boolean
  loadingRequests: boolean
  loadingMoreRequests: boolean
  refreshingPage: boolean
  initializationFailure: { message: string; timedOut: boolean } | null
}

export const initialNavigationState: NavigationState = {
  pendingRequests: [],
  requests: [],
  hostSessions: [],
  hostSessionFactsStatus: 'pending',
  hostSessionFactsRevision: 0,
  hostProfiles: {},
  selectedHostId: null,
  selectedHostSessionId: null,
  requestSearch: '',
  requestFilters: DEFAULT_REQUEST_FILTERS,
  nextRequestCursor: null,
  loadingNavigation: true,
  loadingRequests: true,
  loadingMoreRequests: false,
  refreshingPage: false,
  initializationFailure: null,
}

import type { FeedbackRequestSummary } from '../feedback'

export type NavigationScope = Readonly<{
  hostId: string | null
  hostSessionId: string | null
}>

/** A queried scope remains a candidate until the workspace can accept it. */
export type PreparedNavigationScope = Readonly<{
  scope: NavigationScope
  requests: readonly FeedbackRequestSummary[]
  nextRequestCursor: string | null
}>

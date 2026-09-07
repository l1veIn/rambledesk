import type { FeedbackRequestSummary, HostSessionSummary } from '$lib/generated/feedback'
import { agentSessionViewDescriptor, sessionViewDescriptor, type AgentSessionViewDescriptor, type SessionViewDescriptor } from './viewDescriptors'

/** Request ownership comes from the durable binding, never a display host/session pair. */
export function agentViewForRequest(
  request: Pick<FeedbackRequestSummary, 'managed_session_id'> | null | undefined,
): AgentSessionViewDescriptor | null {
  return request?.managed_session_id ? agentSessionViewDescriptor(request.managed_session_id) : null
}

export function agentSessionForView(
  view: AgentSessionViewDescriptor | null,
  sessions: readonly HostSessionSummary[],
): HostSessionSummary | undefined {
  return view ? sessions.find((session) => session.session_id === view.sessionId && session.management.kind === 'managed') : undefined
}

/** Restoring a closed feedback flow must not implicitly launch its Agent. Requests are newest first. */
export function cancelledFeedbackRestoreTarget(
  session: HostSessionSummary,
  requests: readonly FeedbackRequestSummary[],
): { view: SessionViewDescriptor; requestId: string } | null {
  const latest = requests[0]
  if (session.management.kind !== 'managed' || session.pending_count > 0
    || latest?.status !== 'cancelled' || latest.managed_session_id !== session.session_id
    || latest.host_id !== session.host_id || latest.host_session_id !== session.host_session_id) return null
  return { view: sessionViewDescriptor(session.host_id, session.host_session_id), requestId: latest.request_id }
}

/** Empty Ramble session pages still provide a way to open their Agent conversation. */
export function agentViewForEmptyRamble(
  view: SessionViewDescriptor | null,
  sessions: readonly HostSessionSummary[],
): AgentSessionViewDescriptor | null {
  const session = view ? sessions.find((candidate) => candidate.management.kind === 'managed'
    && candidate.host_id === view.hostId && candidate.host_session_id === view.hostSessionId) : undefined
  return session ? agentSessionViewDescriptor(session.session_id) : null
}

import type { FeedbackRequestSummary, HostSessionSummary } from '$lib/generated/feedback'
import { agentSessionViewDescriptor, type AgentSessionViewDescriptor, type SessionViewDescriptor } from './viewDescriptors'

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

/** Empty Ramble session pages still provide a way to open their Agent conversation. */
export function agentViewForEmptyRamble(
  view: SessionViewDescriptor | null,
  sessions: readonly HostSessionSummary[],
): AgentSessionViewDescriptor | null {
  const session = view ? sessions.find((candidate) => candidate.management.kind === 'managed'
    && candidate.host_id === view.hostId && candidate.host_session_id === view.hostSessionId) : undefined
  return session ? agentSessionViewDescriptor(session.session_id) : null
}

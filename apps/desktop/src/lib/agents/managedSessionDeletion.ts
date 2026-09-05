import type { ApplicationTransport } from '$lib/application/applicationTransport'
import type { HostSessionSummary } from '$lib/generated/feedback'
import { sessionPromptDrafts } from './managedSessionUi'
import { agentSessionViewDescriptor, requestTaskViewDescriptor, sessionViewDescriptor, workspaceViewKey } from '$lib/workspace/viewDescriptors'
import { workspaceShellReducer, type WorkspaceShellState } from '$lib/workspace/workspaceShell'

export function removeManagedSessionViews(
  shell: WorkspaceShellState,
  session: Pick<HostSessionSummary, 'session_id' | 'host_id' | 'host_session_id'>,
  requestIds: readonly string[],
) {
  const closedViewKeys = [workspaceViewKey(agentSessionViewDescriptor(session.session_id)),
    workspaceViewKey(sessionViewDescriptor(session.host_id, session.host_session_id)),
    ...requestIds.map((id) => workspaceViewKey(requestTaskViewDescriptor(id)))]
  const closedActive = shell.activeViewKey !== null && closedViewKeys.includes(shell.activeViewKey)
  return {
    shell: closedViewKeys.reduce((current, viewKey) => workspaceShellReducer(current, { type: 'close', viewKey }), shell),
    closedActive,
    closedViewKeys,
  }
}

/** Archive status is an external-session concern; managed deletion owns runtime cleanup. */
export async function deleteSessionRecord(transport: ApplicationTransport, session: HostSessionSummary): Promise<void> {
  if (session.management.kind === 'managed') {
    await transport.call('deleteManagedSession', { session_id: session.session_id })
    sessionPromptDrafts.forgetSession(session.session_id)
  } else {
    await transport.call('deleteHostSession', { host_id: session.host_id, host_session_id: session.host_session_id })
  }
}

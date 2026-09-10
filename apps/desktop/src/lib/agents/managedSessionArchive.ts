import { agentSessionViewDescriptor, workspaceViewKey } from '$lib/workspace/viewDescriptors'
import { workspaceShellReducer, type WorkspaceShellState } from '$lib/workspace/workspaceShell'

export function removeArchivedManagedSessionView(
  shell: WorkspaceShellState,
  sessionId: string,
  pendingViewKey: string | null,
): {
  shell: WorkspaceShellState
  shouldNavigateToArchive: boolean
  shouldInvalidatePending: boolean
} {
  const viewKey = workspaceViewKey(agentSessionViewDescriptor(sessionId))
  const shouldInvalidatePending = pendingViewKey === viewKey
  return {
    // Archiving only removes the tab; the session and its prompt draft remain available.
    shell: workspaceShellReducer(shell, { type: 'close', viewKey }),
    shouldNavigateToArchive: shell.activeViewKey === viewKey
      && (pendingViewKey === null || shouldInvalidatePending),
    shouldInvalidatePending,
  }
}

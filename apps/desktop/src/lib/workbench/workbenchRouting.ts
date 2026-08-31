import type { SessionOrigin } from '../components/navigation/sessionRailItem'

export type WorkbenchRequestOwner = {
  key: string
  origin: SessionOrigin
  requestId: string
  sessionId: string
}

export type WorkbenchOperationTarget = 'workspace' | 'ramble'

export function ownerForOperation(
  requestId: string,
  target: WorkbenchOperationTarget,
  workspaceOwner: WorkbenchRequestOwner | null,
  rambleOwner: WorkbenchRequestOwner | null,
): WorkbenchRequestOwner | null {
  const preferred = target === 'ramble' ? rambleOwner : workspaceOwner
  return preferred?.requestId === requestId ? preferred : null
}

export type WorkspaceLoadToken = {
  generation: number
  requestKey: string | null
}

export function createWorkspaceLoadGate() {
  let generation = 0

  return {
    begin(requestKey: string | null): WorkspaceLoadToken {
      return { generation: ++generation, requestKey }
    },
    invalidate() {
      generation += 1
    },
    isCurrent(token: WorkspaceLoadToken, activeRequestKey: string | null): boolean {
      return token.generation === generation &&
        (token.requestKey === null || token.requestKey === activeRequestKey)
    },
  }
}

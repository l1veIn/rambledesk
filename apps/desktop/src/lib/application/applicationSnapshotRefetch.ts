import type { ApplicationResourceKey } from '../generated/feedback'
import { applicationResourceKeyIdentity } from './applicationEvents'

export type ApplicationSnapshotRefetchIntent = Readonly<{
  resources: readonly ApplicationResourceKey[]
  isCurrent: () => boolean
}>

export type ApplicationSnapshotRefetchAdapter = Readonly<{
  refetch: (intent: ApplicationSnapshotRefetchIntent) => Promise<void>
  reportError?: (cause: unknown) => void
}>

export function createApplicationSnapshotRefetch(
  adapter: ApplicationSnapshotRefetchAdapter,
) {
  const pending = new Map<string, ApplicationResourceKey>()
  let generation = 0
  let running = false
  let scheduled = false
  let disposed = false

  function request(resources: readonly ApplicationResourceKey[]): void {
    if (disposed) return
    for (const resource of resources) {
      pending.set(applicationResourceKeyIdentity(resource), resource)
    }
    if (running || scheduled || pending.size === 0) return
    scheduled = true
    queueMicrotask(() => {
      scheduled = false
      void drain()
    })
  }

  async function drain(): Promise<void> {
    if (running || disposed) return
    running = true
    try {
      while (!disposed && pending.size > 0) {
        const resources = [...pending.values()]
        pending.clear()
        const intentGeneration = generation
        try {
          await adapter.refetch({
            resources,
            isCurrent: () => !disposed && intentGeneration === generation,
          })
        } catch (cause) {
          if (!disposed && intentGeneration === generation) adapter.reportError?.(cause)
        }
      }
    } finally {
      running = false
      if (!disposed && pending.size > 0) request([])
    }
  }

  function invalidate(): void {
    generation += 1
    pending.clear()
  }

  function dispose(): void {
    disposed = true
    invalidate()
  }

  return { request, invalidate, dispose }
}

export function applicationResourcesAffectNavigation(
  resources: readonly ApplicationResourceKey[],
): boolean {
  return resources.some((resource) =>
    ['all', 'navigation', 'host_session_resources'].includes(resource.kind),
  )
}

export function applicationResourcesRequireFullNavigationSnapshot(
  resources: readonly ApplicationResourceKey[],
): boolean {
  return resources.some((resource) => resource.kind === 'all')
}

export function applicationResourcesAffectWorkspace(
  resources: readonly ApplicationResourceKey[],
  workspace: Readonly<{
    requestId: string
    hostId: string
    hostSessionId: string
  }>,
): boolean {
  return resources.some((resource) => {
    switch (resource.kind) {
      case 'all':
        return true
      case 'navigation':
        return false
      case 'host_session_resources':
        return (
          resource.host_id === workspace.hostId &&
          resource.host_session_id === workspace.hostSessionId
        )
      case 'feedback_workspace':
      case 'published_feedback':
        return resource.request_id === workspace.requestId
    }
  })
}

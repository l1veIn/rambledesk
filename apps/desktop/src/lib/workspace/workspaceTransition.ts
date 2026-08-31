import type { WorkspaceViewDescriptor } from './viewDescriptors'

export type WorkspaceShellIntent =
  | Readonly<{ type: 'open' }>
  | Readonly<{ type: 'close'; viewKey: string }>

export type WorkspaceTransitionTarget = Readonly<{
  view: WorkspaceViewDescriptor | null
  requestId: string | null
  shellAction: WorkspaceShellIntent
  pendingViewKey: string
}>

export type WorkspaceTransitionOutcome =
  | 'activated'
  | 'blocked'
  | 'failed'
  | 'stale'

export type WorkspaceTransitionAdapter<LoadedWorkspace> = {
  saveCurrent: () => Promise<boolean>
  unmountCurrent: () => void
  loadTarget: (target: WorkspaceTransitionTarget) => Promise<LoadedWorkspace | null>
  commitTarget: (
    target: WorkspaceTransitionTarget,
    loaded: LoadedWorkspace | null,
  ) => void
  restoreCurrent: () => void
  setPendingTarget: (target: WorkspaceTransitionTarget | null) => void
  reportFailure: (cause: unknown) => void
}

export function createWorkspaceTransition<LoadedWorkspace>(
  adapter: WorkspaceTransitionAdapter<LoadedWorkspace>,
) {
  let latestIntent = 0
  let transitionQueue: Promise<void> = Promise.resolve()

  function activate(
    target: WorkspaceTransitionTarget,
  ): Promise<WorkspaceTransitionOutcome> {
    const intent = ++latestIntent
    adapter.setPendingTarget(target)

    const result = transitionQueue.then(async (): Promise<WorkspaceTransitionOutcome> => {
      if (intent !== latestIntent) return 'stale'

      try {
        const saved = await adapter.saveCurrent()
        if (intent !== latestIntent) return 'stale'
        if (!saved) {
          adapter.restoreCurrent()
          return 'blocked'
        }

        adapter.unmountCurrent()
        const loaded = target.requestId ? await adapter.loadTarget(target) : null
        if (intent !== latestIntent) return 'stale'

        adapter.commitTarget(target, loaded)
        return 'activated'
      } catch (cause) {
        if (intent !== latestIntent) return 'stale'
        adapter.restoreCurrent()
        adapter.reportFailure(cause)
        return 'failed'
      } finally {
        if (intent === latestIntent) adapter.setPendingTarget(null)
      }
    })

    transitionQueue = result.then(
      () => undefined,
      () => undefined,
    )
    return result
  }

  function invalidate() {
    latestIntent += 1
    adapter.restoreCurrent()
    adapter.setPendingTarget(null)
  }

  return { activate, invalidate }
}

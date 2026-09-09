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

export type WorkspaceTransitionAdapter<LoadedWorkspace, Target extends WorkspaceTransitionTarget = WorkspaceTransitionTarget> = {
  saveCurrent: () => Promise<boolean>
  unmountCurrent: () => void
  loadTarget: (target: Target) => Promise<LoadedWorkspace | null>
  commitTarget: (
    target: Target,
    loaded: LoadedWorkspace | null,
  ) => void
  restoreCurrent: () => void
  setPendingTarget: (target: Target | null) => void
  reportFailure: (cause: unknown) => void
}

export function createWorkspaceTransition<LoadedWorkspace, Target extends WorkspaceTransitionTarget = WorkspaceTransitionTarget>(
  adapter: WorkspaceTransitionAdapter<LoadedWorkspace, Target>,
) {
  let latestIntent = 0
  let transitionQueue: Promise<void> = Promise.resolve()

  function activate(
    target: Target,
    expectedIntent?: number,
    canLeaveCurrent: () => boolean = () => true,
  ): Promise<WorkspaceTransitionOutcome> {
    // A delayed scope/catalog read cannot supersede a newer user navigation.
    if (expectedIntent !== undefined && expectedIntent !== latestIntent) return Promise.resolve('stale')
    // Scope preparation and activation are parts of one user intent. Reusing its
    // reservation also lets a background refresh yield to later navigation.
    const intent = expectedIntent ?? ++latestIntent
    adapter.setPendingTarget(target)

    const result = transitionQueue.then(async (): Promise<WorkspaceTransitionOutcome> => {
      if (intent !== latestIntent) return 'stale'

      try {
        if (!canLeaveCurrent()) return 'blocked'
        const saved = await adapter.saveCurrent()
        if (intent !== latestIntent) return 'stale'
        if (!saved || !canLeaveCurrent()) {
          adapter.restoreCurrent()
          return 'blocked'
        }

        adapter.unmountCurrent()
        const loaded = target.requestId ? await adapter.loadTarget(target) : null
        if (intent !== latestIntent) return 'stale'
        if (!canLeaveCurrent()) {
          adapter.restoreCurrent()
          return 'blocked'
        }

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
    const intent = ++latestIntent
    adapter.restoreCurrent()
    adapter.setPendingTarget(null)
    return intent
  }

  return { activate, invalidate, currentIntent: () => latestIntent, isCurrent: (intent: number) => intent === latestIntent }
}

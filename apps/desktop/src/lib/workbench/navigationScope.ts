import { get } from 'svelte/store'

import type { WorkspaceTransitionOutcome } from '../workspace/workspaceTransition'
import type { createNavigationController } from './navigationController'

type NavigationController = ReturnType<typeof createNavigationController>

export type NavigationScope = Readonly<{
  hostId: string | null
  hostSessionId: string | null
}>

export function currentNavigationScope(navigation: NavigationController): NavigationScope {
  const state = get(navigation)
  return { hostId: state.selectedHostId, hostSessionId: state.selectedHostSessionId }
}

/** A blocked or failed transition must not leave the rail showing the wrong scope. */
export async function restoreNavigationScope(
  navigation: NavigationController,
  scope: NavigationScope,
  outcome: WorkspaceTransitionOutcome,
) {
  if (outcome !== 'blocked' && outcome !== 'failed') return
  await navigation.selectScope(scope.hostId, scope.hostSessionId)
}

export type WorkspaceTabNavigationIntent = 'first' | 'last' | 'next' | 'previous'

export type WorkspaceTabKeyboardAction =
  | Readonly<{ type: 'activate' }>
  | Readonly<{ type: 'close' }>
  | Readonly<{ type: 'move'; intent: WorkspaceTabNavigationIntent }>

export function workspaceTabId(viewKey: string): string {
  return `workspace-tab-${encodeURIComponent(viewKey)}`
}

export function workspaceTabPanelId(viewKey: string): string {
  return `workspace-tabpanel-${encodeURIComponent(viewKey)}`
}

export function workspaceTabKeyboardAction(key: string): WorkspaceTabKeyboardAction | null {
  if (key === 'ArrowLeft') return { type: 'move', intent: 'previous' }
  if (key === 'ArrowRight') return { type: 'move', intent: 'next' }
  if (key === 'Home') return { type: 'move', intent: 'first' }
  if (key === 'End') return { type: 'move', intent: 'last' }
  if (key === 'Enter' || key === ' ') return { type: 'activate' }
  if (key === 'Delete') return { type: 'close' }
  return null
}

export function requestWorkspaceTabActivation(
  viewKey: string,
  blocked: boolean,
  activate: (viewKey: string) => void,
): boolean {
  if (blocked) return false
  activate(viewKey)
  return true
}

export function workspaceTabNavigationTarget(
  viewKeys: readonly string[],
  currentViewKey: string | null,
  intent: WorkspaceTabNavigationIntent,
): string | null {
  if (viewKeys.length === 0) return null
  if (intent === 'first') return viewKeys[0]
  if (intent === 'last') return viewKeys[viewKeys.length - 1]

  const currentIndex = currentViewKey ? viewKeys.indexOf(currentViewKey) : -1
  if (currentIndex === -1) {
    return intent === 'previous' ? viewKeys[viewKeys.length - 1] : viewKeys[0]
  }

  const offset = intent === 'next' ? 1 : -1
  return viewKeys[(currentIndex + offset + viewKeys.length) % viewKeys.length]
}

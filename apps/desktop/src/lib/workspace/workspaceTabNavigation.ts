export type WorkspaceTabNavigationIntent = 'first' | 'last' | 'next' | 'previous'

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

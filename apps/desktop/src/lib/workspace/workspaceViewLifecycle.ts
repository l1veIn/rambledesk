import type { WorkspaceViewDescriptor } from './viewDescriptors'

export function leavesSettingsView(
  previous: WorkspaceViewDescriptor | null,
  next: WorkspaceViewDescriptor | null,
): boolean {
  return previous?.kind === 'settings' && next?.kind !== 'settings'
}

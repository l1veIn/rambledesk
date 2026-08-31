export type SessionViewDescriptor = Readonly<{
  kind: 'session'
  hostId: string
  hostSessionId: string
}>

export type SettingsViewDescriptor = Readonly<{
  kind: 'settings'
}>

export type WorkspaceViewDescriptor = SessionViewDescriptor | SettingsViewDescriptor

export function sessionViewDescriptor(
  hostId: string,
  hostSessionId: string,
): SessionViewDescriptor {
  return { kind: 'session', hostId, hostSessionId }
}

export function settingsViewDescriptor(): SettingsViewDescriptor {
  return { kind: 'settings' }
}

export function workspaceViewKey(view: WorkspaceViewDescriptor): string {
  if (view.kind === 'settings') return 'settings:singleton'
  return `${view.kind}:${JSON.stringify([view.hostId, view.hostSessionId])}`
}

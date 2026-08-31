export type SessionViewDescriptor = Readonly<{
  kind: 'session'
  hostId: string
  hostSessionId: string
}>

export type SettingsViewDescriptor = Readonly<{
  kind: 'settings'
}>

export type RequestTaskViewDescriptor = Readonly<{
  kind: 'request-task'
  requestId: string
}>

export type RambelleProfileViewDescriptor = Readonly<{
  kind: 'rambelle-profile'
}>

export type WorkspaceViewDescriptor =
  | SessionViewDescriptor
  | SettingsViewDescriptor
  | RequestTaskViewDescriptor
  | RambelleProfileViewDescriptor

export function sessionViewDescriptor(
  hostId: string,
  hostSessionId: string,
): SessionViewDescriptor {
  return { kind: 'session', hostId, hostSessionId }
}

export function settingsViewDescriptor(): SettingsViewDescriptor {
  return { kind: 'settings' }
}

export function requestTaskViewDescriptor(requestId: string): RequestTaskViewDescriptor {
  return { kind: 'request-task', requestId }
}

export function rambelleProfileViewDescriptor(): RambelleProfileViewDescriptor {
  return { kind: 'rambelle-profile' }
}

export function workspaceViewKey(view: WorkspaceViewDescriptor): string {
  switch (view.kind) {
    case 'settings':
      return 'settings:singleton'
    case 'request-task':
      return `${view.kind}:${JSON.stringify(view.requestId)}`
    case 'rambelle-profile':
      return 'rambelle-profile:singleton'
    case 'session':
      return `${view.kind}:${JSON.stringify([view.hostId, view.hostSessionId])}`
  }
}

export type SessionViewDescriptor = Readonly<{
  kind: 'session'
  hostId: string
  hostSessionId: string
}>

export type AgentSessionViewDescriptor = Readonly<{
  kind: 'agent-session'
  sessionId: string
}>

export type AgentDraftViewDescriptor = Readonly<{
  kind: 'agent-draft'
  draftId: string
}>

export type SettingsViewDescriptor = Readonly<{
  kind: 'settings'
}>

export type InboxViewDescriptor = Readonly<{
  kind: 'inbox'
}>

export type ArchiveViewDescriptor = Readonly<{
  kind: 'archive'
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
  | AgentSessionViewDescriptor
  | AgentDraftViewDescriptor
  | InboxViewDescriptor
  | ArchiveViewDescriptor
  | SettingsViewDescriptor
  | RequestTaskViewDescriptor
  | RambelleProfileViewDescriptor

export function sessionViewDescriptor(
  hostId: string,
  hostSessionId: string,
): SessionViewDescriptor {
  return { kind: 'session', hostId, hostSessionId }
}

export function agentSessionViewDescriptor(sessionId: string): AgentSessionViewDescriptor {
  return { kind: 'agent-session', sessionId }
}

export function agentDraftViewDescriptor(draftId: string): AgentDraftViewDescriptor {
  return { kind: 'agent-draft', draftId }
}

export function settingsViewDescriptor(): SettingsViewDescriptor {
  return { kind: 'settings' }
}

export function inboxViewDescriptor(): InboxViewDescriptor {
  return { kind: 'inbox' }
}

export function archiveViewDescriptor(): ArchiveViewDescriptor {
  return { kind: 'archive' }
}

export function requestTaskViewDescriptor(requestId: string): RequestTaskViewDescriptor {
  return { kind: 'request-task', requestId }
}

export function rambelleProfileViewDescriptor(): RambelleProfileViewDescriptor {
  return { kind: 'rambelle-profile' }
}

export function workspaceViewKey(view: WorkspaceViewDescriptor): string {
  switch (view.kind) {
    case 'agent-draft':
      return `${view.kind}:${JSON.stringify(view.draftId)}`
    case 'agent-session':
      return `${view.kind}:${JSON.stringify(view.sessionId)}`
    case 'inbox':
      return 'inbox:singleton'
    case 'archive':
      return 'archive:singleton'
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

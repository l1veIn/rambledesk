export type SessionViewDescriptor = Readonly<{
  kind: 'session'
  hostId: string
  hostSessionId: string
}>

export type WorkspaceViewDescriptor = SessionViewDescriptor

export function sessionViewDescriptor(
  hostId: string,
  hostSessionId: string,
): SessionViewDescriptor {
  return { kind: 'session', hostId, hostSessionId }
}

export function workspaceViewKey(view: SessionViewDescriptor): string {
  return `${view.kind}:${JSON.stringify([view.hostId, view.hostSessionId])}`
}

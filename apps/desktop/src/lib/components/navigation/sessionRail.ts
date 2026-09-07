import type { HostSessionSummary } from '$lib/feedback'
import { sessionViewDescriptor, workspaceViewKey } from '$lib/workspace/viewDescriptors'

export function hostSessionKey(session: HostSessionSummary): string {
  return workspaceViewKey(sessionViewDescriptor(session.host_id, session.host_session_id))
}

export function orderSessionRailSessions(
  sessions: readonly HostSessionSummary[],
): HostSessionSummary[] {
  return [...sessions].sort((left, right) => {
    return (
      compareNullableIsoDesc(left.pinned_at, right.pinned_at) ||
      (right.created_at || right.updated_at).localeCompare(left.created_at || left.updated_at) ||
      left.host_id.localeCompare(right.host_id) ||
      left.host_session_id.localeCompare(right.host_session_id)
    )
  })
}

export type SessionProject = {
  key: string
  kind: 'project' | 'unassigned' | 'external'
  cwd: string | null
  name: string | null
  sessions: HostSessionSummary[]
  updatedAt: string
  pendingCount: number
}

/** Compare Windows paths independently of the OS rendering this history. */
export function sessionProjectPath(cwd: string | null | undefined): {
  key: string
  cwd: string
  name: string
} | null {
  const value = cwd?.trim()
  if (!value) return null
  const windows = /^[a-z]:[\\/]/i.test(value) || /^(\\\\|\/\/)/.test(value)
  let path = windows ? value.replace(/\\/g, '/') : value
  path = path.replace(/\/{2,}/g, '/')
  if (windows && /^(\\\\|\/\/)/.test(value)) path = `/${path}`
  path = path.replace(/\/+$/, '') || '/'
  if (/^[a-z]:$/i.test(path)) path += '/'
  const key = windows ? path.toLowerCase() : path
  const name = path.split('/').filter(Boolean).at(-1) ?? path
  return { key: `project:${key}`, cwd: value, name }
}

export function groupSessionRailProjects(sessions: readonly HostSessionSummary[]): SessionProject[] {
  const groups = new Map<string, SessionProject>()
  for (const session of sessions) {
    const cwd = session.cwd
      ?? (session.management.kind === 'managed' ? session.management.cwd : null)
    const path = session.management.kind === 'managed' ? sessionProjectPath(cwd) : null
    const kind = session.management.kind === 'external' ? 'external' : path ? 'project' : 'unassigned'
    const key = path?.key ?? `group:${kind}`
    let project = groups.get(key)
    if (!project) {
      project = {
        key, kind, cwd: path?.cwd ?? null, name: path?.name ?? null,
        sessions: [], updatedAt: session.updated_at, pendingCount: 0,
      }
      groups.set(key, project)
    }
    project.sessions.push(session)
    project.pendingCount += session.pending_count
    if (session.updated_at > project.updatedAt) project.updatedAt = session.updated_at
  }
  return [...groups.values()]
    .map((project) => ({ ...project, sessions: orderSessionRailSessions(project.sessions) }))
    .sort((left, right) =>
      projectKindOrder[left.kind] - projectKindOrder[right.kind]
      || right.updatedAt.localeCompare(left.updatedAt)
      || left.key.localeCompare(right.key),
    )
}

const projectKindOrder: Record<SessionProject['kind'], number> = { project: 0, unassigned: 1, external: 2 }

export function filterSessionRailProjects(projects: readonly SessionProject[], search: string): SessionProject[] {
  const query = search.trim().toLocaleLowerCase()
  if (!query) return [...projects]
  return projects.flatMap((project) => {
    if (`${project.name ?? ''} ${project.cwd ?? ''}`.toLocaleLowerCase().includes(query)) return [project]
    const sessions = project.sessions.filter((session) =>
      `${session.title} ${session.cwd ?? ''}`.toLocaleLowerCase().includes(query),
    )
    return sessions.length ? [{ ...project, sessions }] : []
  })
}

function compareNullableIsoDesc(
  left: string | null | undefined,
  right: string | null | undefined,
): number {
  if (left === right) return 0
  if (!left) return 1
  if (!right) return -1
  return right.localeCompare(left)
}

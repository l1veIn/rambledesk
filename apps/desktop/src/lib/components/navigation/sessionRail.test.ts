import { describe, expect, it } from 'vitest'

import type { HostSessionSummary } from '$lib/feedback'
import { filterSessionRailProjects, groupSessionRailProjects, hostSessionKey, orderSessionRailSessions, sessionProjectPath } from './sessionRail'

function session(
  hostId: string,
  hostSessionId: string,
  updatedAt: string,
  pinnedAt: string | null = null,
  hostPinnedAt: string | null = null,
): HostSessionSummary {
  return {
    session_id: `local:${hostId}:${hostSessionId}`,
    management: { kind: 'external' },
    host_id: hostId,
    host_session_id: hostSessionId,
    title: hostSessionId,
    source_hint: null,
    request_count: 1,
    pending_count: 0,
    updated_at: updatedAt,
    pinned_at: pinnedAt,
    archived_at: null,
    host_pinned_at: hostPinnedAt,
  }
}

describe('orderSessionRailSessions', () => {
  it('keeps creation order when an older session receives activity or is renamed', () => {
    const older = { ...inProject('claude', 'older', '/work/repo', '2026-09-07T12:00:00Z'), created_at: '2026-09-01T10:00:00Z' }
    const newer = { ...inProject('codex', 'newer', '/work/repo', '2026-09-03T10:00:00Z'), created_at: '2026-09-03T10:00:00Z' }
    const order = (entries: HostSessionSummary[]) => groupSessionRailProjects(entries)[0].sessions.map(entry => entry.host_session_id)
    expect(order([older, newer])).toEqual(['newer', 'older'])
    expect(order([{ ...older, title: 'Renamed', updated_at: '2026-09-08T10:00:00Z' }, newer])).toEqual(['newer', 'older'])
    expect(order([{ ...older, pinned_at: '2026-09-07T12:00:00Z' }, newer])).toEqual(['older', 'newer'])
  })

  it('orders the flat rail globally by session pin and recency', () => {
    const sessions = [
      session('pi', 'latest-unpinned', '2026-09-01T10:00:00Z'),
      session('codex', 'older-pinned', '2026-08-30T10:00:00Z', '2026-08-31T10:00:00Z'),
      session('claude', 'latest-pinned', '2026-08-29T10:00:00Z', '2026-09-01T11:00:00Z'),
      session('codex', 'older-unpinned', '2026-08-28T10:00:00Z'),
    ]

    expect(orderSessionRailSessions(sessions).map(hostSessionKey)).toEqual([
      'session:["claude","latest-pinned"]',
      'session:["codex","older-pinned"]',
      'session:["pi","latest-unpinned"]',
      'session:["codex","older-unpinned"]',
    ])
    expect(sessions.map((entry) => entry.host_session_id)).toEqual([
      'latest-unpinned',
      'older-pinned',
      'latest-pinned',
      'older-unpinned',
    ])
  })

  it('uses host and session ids as deterministic tie breakers', () => {
    const timestamp = '2026-09-01T10:00:00Z'
    const sessions = [
      session('pi', 'shared', timestamp),
      session('codex', 'zeta', timestamp),
      session('codex', 'alpha', timestamp),
      session('codex', 'shared', timestamp),
    ]

    expect(orderSessionRailSessions(sessions).map(hostSessionKey)).toEqual([
      'session:["codex","alpha"]',
      'session:["codex","shared"]',
      'session:["codex","zeta"]',
      'session:["pi","shared"]',
    ])
    expect(hostSessionKey(sessions[0])).not.toBe(hostSessionKey(sessions[3]))
  })

  it('ignores historical host pins while preserving explicit session pins', () => {
    const sessions = [
      session('pi', 'pinned-session', '2026-09-01T10:00:00Z', '2026-09-01T11:00:00Z'),
      session('codex', 'older-host-session', '2026-08-29T10:00:00Z', null, '2026-09-01T12:00:00Z'),
      session('codex', 'newer-host-session', '2026-08-30T10:00:00Z', null, '2026-09-01T12:00:00Z'),
    ]

    expect(orderSessionRailSessions(sessions).map(hostSessionKey)).toEqual([
      'session:["pi","pinned-session"]',
      'session:["codex","newer-host-session"]',
      'session:["codex","older-host-session"]',
    ])
  })
})

function inProject(hostId: string, id: string, cwd: string | null, updated = '2026-09-01T10:00:00Z'): HostSessionSummary {
  return {
    ...session(hostId, id, updated), cwd: cwd ?? undefined,
    management: { kind: 'managed', protocol: 'acp', agent_config_id: 'agent', cwd: cwd ?? '', remote_session_id: null },
  }
}

describe('project session rail', () => {
  it('groups agents together by directory and merges Windows case and separator variants', () => {
    const projects = groupSessionRailProjects([
      inProject('codex', 'one', 'D:\\Projects\\RambleDesk\\'),
      inProject('claude', 'two', 'd:/projects/rambledesk'),
      inProject('codex', 'three', 'D:/Other/RambleDesk'),
    ])
    expect(projects).toHaveLength(2)
    expect(projects.find((project) => project.sessions.length === 2)?.sessions.map((entry) => entry.host_id).sort()).toEqual(['claude', 'codex'])
    expect(projects.every((project) => project.name?.toLowerCase() === 'rambledesk')).toBe(true)
    expect(projects[0].key).not.toBe(projects[1].key)
  })

  it('uses older managed directories and separates external sessions from missing-directory managed sessions', () => {
    const managed = session('claude', 'managed', '2026-09-01T10:00:00Z')
    managed.management = { kind: 'managed', protocol: 'acp', agent_config_id: 'agent', cwd: '/work/repo', remote_session_id: null }
    const legacy = session('codex', 'legacy', '2026-09-04T10:00:00Z')
    const externalWithDirectory = { ...session('claude', 'external-with-directory', '2026-09-03T10:00:00Z'), cwd: '/work/repo' }
    externalWithDirectory.pinned_at = '2026-09-04T10:00:00Z'
    const projects = groupSessionRailProjects([
      legacy, externalWithDirectory,
      inProject('pi', 'missing-directory', null, '2026-09-02T10:00:00Z'), managed,
    ])
    expect(projects.map((project) => project.kind)).toEqual(['project', 'unassigned', 'external'])
    expect(projects.map((project) => project.cwd)).toEqual(['/work/repo', null, null])
    expect(projects[0].sessions.map((entry) => entry.host_session_id)).toEqual(['managed'])
    expect(projects[1].sessions.map((entry) => entry.host_session_id)).toEqual(['missing-directory'])
    expect(projects[2].sessions.map((entry) => entry.host_session_id)).toEqual(['external-with-directory', 'legacy'])
    expect(new Set(projects.map((project) => project.key)).size).toBe(3)
    expect(filterSessionRailProjects(projects, '/work/repo').map((project) => project.kind)).toEqual(['project', 'external'])
    expect(filterSessionRailProjects(projects, 'legacy')[0].kind).toBe('external')
  })

  it('keeps Unix case-sensitive directories distinct and handles drive and share roots', () => {
    expect(sessionProjectPath('/work/Repo/')?.key).not.toBe(sessionProjectPath('/work/repo')?.key)
    expect(sessionProjectPath('C:\\')).toEqual({ key: 'project:c:/', cwd: 'C:\\', name: 'C:' })
    expect(sessionProjectPath('\\\\Server\\Share\\Repo')?.key).toBe('project://server/share/repo')
    expect(sessionProjectPath('//server/share/Repo/')?.key).toBe('project://server/share/repo')
    expect(sessionProjectPath('/')).toEqual({ key: 'project:/', cwd: '/', name: '/' })
    expect(sessionProjectPath('  ')).toBeNull()
  })

  it('orders projects by their latest activity and sessions by pin then recency without mutating source', () => {
    const older = inProject('claude', 'pinned', '/work/a', '2026-08-01T10:00:00Z')
    older.pinned_at = '2026-09-01T10:00:00Z'
    const sessions = [older, inProject('codex', 'recent', '/work/a'), inProject('pi', 'newest', '/work/b', '2026-09-02T10:00:00Z')]
    const projects = groupSessionRailProjects(sessions)
    expect(projects.map((project) => project.name)).toEqual(['b', 'a'])
    expect(projects[1].sessions.map((entry) => entry.host_session_id)).toEqual(['pinned', 'recent'])
    expect(sessions.map((entry) => entry.host_session_id)).toEqual(['pinned', 'recent', 'newest'])
  })

  it('searches project names, paths, and ACP titles without depending on feedback requests', () => {
    const projects = groupSessionRailProjects([
      inProject('claude', 'Fix startup', '/work/RambleDesk'),
      inProject('codex', 'Build composer', '/work/RambleDesk'),
      inProject('pi', 'Fix permissions', '/work/Other'),
      inProject('codex', 'Historical title', null),
    ])
    expect(filterSessionRailProjects(projects, ' RAMBLE ').flatMap((project) => project.sessions)).toHaveLength(2)
    expect(filterSessionRailProjects(projects, '/work/Other')).toHaveLength(1)
    expect(filterSessionRailProjects(projects, 'fix').flatMap((project) => project.sessions)).toHaveLength(2)
    expect(filterSessionRailProjects(projects, 'Historical')[0].cwd).toBeNull()
    expect(filterSessionRailProjects(projects, 'not found')).toEqual([])
    expect(projects[1].sessions).toHaveLength(2)
  })
})

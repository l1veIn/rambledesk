import { describe, expect, it, vi } from 'vitest'
import { render } from 'svelte/server'
import HostSessionRail from './HostSessionRail.svelte'
import type { HostSessionSummary } from '$lib/feedback'

vi.mock('$lib/preferences', async () => {
  const { writable } = await import('svelte/store')
  return { locale: writable('en') }
})

function session(id: string, cwd: string | null, managed = true): HostSessionSummary {
  return {
    session_id: id, host_id: 'claude', host_session_id: id, title: id, cwd: cwd ?? undefined,
    management: managed ? { kind: 'managed', protocol: 'acp', agent_config_id: 'agent', cwd: cwd!, remote_session_id: null } : { kind: 'external' },
    source_hint: null, request_count: 0, pending_count: 0,
    updated_at: '2026-09-01T10:00:00Z', pinned_at: null, archived_at: null, host_pinned_at: null,
  }
}

function rail(sessions: HostSessionSummary[], extra = {}) {
  return render(HostSessionRail, { props: {
    sessions, resolveHostProfile: () => ({ id: 'claude', label: 'Claude', icon_svg: '<svg></svg>', default_adapter: '', continuation_mode: 'none' }),
    onNewSession: vi.fn(), ...extra,
  } }).body
}

describe('project sidebar rendering', () => {
  it('renders project hierarchy, contextual creation, and only pin/archive session actions', () => {
    const html = rail([session('First task', 'D:/work/repo'), session('Second task', 'D:/work/repo'), session('Legacy task', null, false)])
    expect(html).toContain('data-project-key="project:d:/work/repo"')
    expect(html).toContain('aria-label="New session in repo"')
    expect(html).toContain('aria-label="External sessions"')
    expect(html).toContain('aria-label="Pin session: First task"')
    expect(html).toContain('aria-label="Archive session: First task"')
    expect(html).not.toContain('title="External session"')
    expect(html).not.toContain('Rename session')
    expect(html).not.toContain('Delete session')
    expect(html).not.toContain('Pin host')
    expect(html).not.toContain('Session actions')
  })

  it('keeps search and inbox as icon controls without a permanently visible search input', () => {
    const html = rail([])
    expect(html).toContain('aria-label="Search sessions and projects"')
    expect(html).toContain('aria-label="All requests"')
    expect(html).not.toContain('<input')
    expect(html).toContain('No project sessions yet')
  })

  it('keeps pending sessions unarchivable and exposes current selection and unpin', () => {
    const pending = { ...session('Pending', '/work/repo'), pending_count: 1, pinned_at: '2026-09-01T10:00:00Z' }
    const html = rail([pending], { activeHostId: 'claude', activeHostSessionId: 'Pending' })
    expect(html).toContain('aria-current="page"')
    expect(html).toContain('aria-label="Unpin session: Pending"')
    expect(html).toMatch(/<button[^>]*disabled[^>]*aria-label="Archive session: Pending"|<button[^>]*aria-label="Archive session: Pending"[^>]*disabled/)
  })

  it('shows agent logos in project/session order without search or inbox when collapsed', () => {
    const sessions = [
      session('Older', '/work/repo'),
      { ...session('Pinned', '/work/repo'), host_id: 'dsh', pinned_at: '2026-09-02T10:00:00Z' },
      session('External', null, false),
    ]
    const html = rail(sessions, { collapsed: true, activeHostId: 'dsh', activeHostSessionId: 'Pinned' })
    expect(html.match(/data-session-id="([^"]+)"/g)).toEqual([
      'data-session-id="Pinned"', 'data-session-id="Older"', 'data-session-id="External"',
    ])
    expect(html).toContain('DeepSeek Harness')
    expect(html).toContain('aria-current="page"')
    expect(html).not.toContain('data-project-key=')
    expect(html).toContain('aria-label="Expand sidebar"')
    expect(html).not.toContain('aria-label="Search sessions and projects"')
    expect(html).not.toContain('aria-label="All requests"')
    expect(html).toContain('aria-label="New session"')
    expect(html).toContain('aria-label="Settings"')
  })

  it('does not highlight inbox or a session when the workspace is a draft or settings', () => {
    const html = rail([session('Existing task', '/work/repo')])
    expect(html).not.toContain('aria-current="page"')
    const inboxButton = html.match(/<button[^>]*aria-label="All requests"[^>]*>/)?.[0]
    expect(inboxButton).toBeDefined()
    expect(inboxButton).not.toContain('bg-sidebar-accent text-sidebar-accent-foreground')
  })

  it('highlights inbox only when explicitly active', () => {
    const html = rail([session('Existing task', '/work/repo')], { inboxActive: true })
    const inboxButton = html.match(/<button[^>]*aria-label="All requests"[^>]*>/)?.[0]
    expect(inboxButton).toContain('aria-current="page"')
    expect(inboxButton).toContain('bg-sidebar-accent text-sidebar-accent-foreground')
    expect(html.match(/aria-current="page"/g)).toHaveLength(1)
  })
})

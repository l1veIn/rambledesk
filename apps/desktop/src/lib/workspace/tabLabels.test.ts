import { describe, expect, it, vi } from 'vitest'

import type { HostSessionSummary } from '$lib/feedback'
import { settingsViewDescriptor, inboxViewDescriptor, sessionViewDescriptor, requestTaskViewDescriptor } from './viewDescriptors'
import { sessionTabLabel, workspaceTabLabel, type TabLabelContext } from './tabLabels'

const session: HostSessionSummary = {
  session_id: 'local-1',
  management: { kind: 'external' },
  host_id: 'codex',
  host_session_id: 'alpha',
  title: 'Review shell',
  source_hint: 'Workbench',
  request_count: 1,
  pending_count: 1,
  updated_at: '2026-09-08T00:00:00Z',
  pinned_at: null,
  archived_at: null,
  host_pinned_at: null,
}

function context(overrides: Partial<TabLabelContext> = {}): TabLabelContext {
  return {
    hostSessions: [session],
    resolveHostProfile: (hostId) => ({ id: hostId, label: 'Codex' } as never),
    taskTabTitles: new Map([['request-1', 'Fix the shell']]),
    locale: 'en',
    tr: (source) => source,
    ...overrides,
  }
}

describe('tab labels', () => {
  it('labels a session tab with the session title and host', () => {
    expect(
      sessionTabLabel(sessionViewDescriptor('codex', 'alpha'), context()),
    ).toBe('Review shell · Codex')
    expect(
      sessionTabLabel(sessionViewDescriptor('codex', 'missing'), context()),
    ).toBe('missing · Codex')
  })

  it('labels fixed views from translations', () => {
    expect(workspaceTabLabel(inboxViewDescriptor(), context())).toBe('All requests')
    expect(workspaceTabLabel(settingsViewDescriptor(), context())).toBe('Settings')
    expect(workspaceTabLabel(sessionViewDescriptor('codex', 'alpha'), context())).toBe(
      'Review shell · Codex',
    )
  })

  it('uses the task title map before the fallback', () => {
    expect(workspaceTabLabel(requestTaskViewDescriptor('request-1'), context())).toBe(
      'Fix the shell',
    )
    expect(workspaceTabLabel(requestTaskViewDescriptor('request-2'), context())).toBe(
      'Task brief',
    )
  })

  it('labels an agent session with its managed session title', () => {
    const managed: HostSessionSummary = {
      ...session,
      session_id: 'local-2',
      management: {
        kind: 'managed',
        protocol: 'acp',
        agent_config_id: 'config-1',
        cwd: '/repo',
        remote_session_id: 'remote-1',
      },
    }
    expect(
      workspaceTabLabel(
        { kind: 'agent-session', sessionId: 'local-2' },
        context({ hostSessions: [managed] }),
      ),
    ).toBe('Review shell · Agent')
    expect(
      workspaceTabLabel({ kind: 'agent-session', sessionId: 'gone' }, context()),
    ).toBe('Agent session')
  })

  it('translates the new-session tab per locale', () => {
    expect(workspaceTabLabel({ kind: 'agent-draft', draftId: 'draft-1' }, context())).toBe(
      'New session',
    )
    expect(
      workspaceTabLabel(
        { kind: 'agent-draft', draftId: 'draft-1' },
        context({ locale: 'zh-CN' }),
      ),
    ).toBe('新建会话')
  })
})

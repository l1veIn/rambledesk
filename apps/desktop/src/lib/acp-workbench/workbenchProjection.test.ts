import { describe, expect, it } from 'vitest'

import type { FeedbackRequestSummary, HostSessionSummary } from '$lib/feedback'
import { sessionRailKey } from '$lib/components/navigation/sessionRailItem'
import type { HostProfile } from '$lib/workbench/types'

import {
  projectAttentionItem,
  projectFeedbackWorkspace,
  projectUnifiedWorkbench,
} from './workbenchProjection'
import type {
  AcpSessionSummary,
  AgentSummary,
  FeedbackAttentionItem,
  PermissionAttentionItem,
} from './types'

const session: AcpSessionSummary = {
  sessionId: 'session-1',
  title: 'Session',
  agentId: 'codex',
  agentLabel: 'Codex',
  workspace: '/workspace',
  model: 'gpt',
  reasoningEffort: 'high',
  accessMode: 'workspace_write',
  status: 'waiting',
  pendingCount: 1,
  pinnedAt: null,
  archivedAt: null,
  updatedAt: '2026-08-30T00:00:00Z',
}

const feedback: FeedbackAttentionItem = {
  id: 'feedback-1',
  sessionId: session.sessionId,
  kind: 'feedback',
  title: 'Review this',
  summary: 'A summary',
  instructions: 'Details',
  actions: ['Open the app'],
  draftDocument: { type: 'doc', content: [] },
  draftMarkdown: 'Draft',
  draftRevision: 2,
  status: 'waiting',
  createdAt: '2026-08-30T00:00:00Z',
}

const adapterSession: HostSessionSummary = {
  host_id: 'codex',
  host_session_id: 'session-1',
  title: 'Legacy Session',
  source_hint: '/adapter/workspace',
  request_count: 1,
  pending_count: 1,
  updated_at: '2026-08-30T01:00:00Z',
  pinned_at: null,
  archived_at: null,
  host_pinned_at: null,
}

const adapterRequest: FeedbackRequestSummary = {
  request_id: 'feedback-1',
  host_id: adapterSession.host_id,
  host_session_id: adapterSession.host_session_id,
  source_hint: adapterSession.source_hint,
  title: 'Legacy review',
  what_happened: 'Check the old Adapter output',
  status: 'waiting',
  resolution: null,
  allow_finish: false,
  final_summary: null,
  revision: 0,
  created_at: '2026-08-30T00:30:00Z',
  updated_at: '2026-08-30T00:30:00Z',
}

const agents: AgentSummary[] = [{
  id: 'codex',
  label: 'Codex ACP',
  iconSvg: '<svg data-acp="true" />',
  supportsStructuredRamble: true,
  models: ['gpt'],
  reasoningEfforts: ['high'],
}]

const resolveHostProfile = (hostId: string): HostProfile => ({
  id: hostId,
  label: 'Legacy Codex Adapter',
  icon_svg: '<svg data-adapter="true" />',
  default_adapter: 'generic_mcp',
  continuation_mode: 'manual',
})

function unified(filter = { sessionKey: null as string | null, search: '' }) {
  return projectUnifiedWorkbench({
    adapterSessions: [adapterSession],
    adapterRequests: [adapterRequest],
    acpSessions: [session],
    attentionItems: [feedback],
    agents,
    resolveHostProfile,
    filter,
  })
}

describe('ACP workbench projection', () => {
  it('projects Feedback into the existing polished Workspace contract', () => {
    const workspace = projectFeedbackWorkspace(feedback, session)
    expect(workspace.request.request_id).toBe('feedback-1')
    expect(workspace.request.host_id).toBe('codex')
    expect(workspace.actions).toEqual([{ id: 'action-1', instruction: 'Open the app' }])
    expect(workspace.draft.saved_revision).toBe(2)
    expect(JSON.parse(workspace.draft.document_json ?? '')).toMatchObject({ type: 'doc' })
  })

  it('projects Permission without pretending it is Feedback', () => {
    const permission: PermissionAttentionItem = {
      id: 'permission-1',
      sessionId: session.sessionId,
      kind: 'permission',
      title: 'Run command?',
      description: 'The Agent wants to run checks.',
      toolTitle: 'Run command',
      command: 'pnpm check',
      path: '/workspace',
      toolCall: {},
      options: [],
      status: 'waiting',
      createdAt: '2026-08-30T00:00:00Z',
    }
    expect(projectAttentionItem(permission, session)).toMatchObject({
      id: 'permission-1',
      kind: 'permission',
      summary: 'The Agent wants to run checks.',
      agentId: 'codex',
    })
  })

  it('projects Adapter and Managed ACP Sessions together without erasing capabilities', () => {
    const projection = unified()

    expect(projection.sessions).toHaveLength(2)
    expect(projection.sessions.find((item) => item.origin === 'adapter')).toMatchObject({
      hostLabel: 'Legacy Codex Adapter',
      requestCount: 1,
      canRename: true,
      canPin: true,
      canArchive: true,
      status: null,
    })
    expect(projection.sessions.find((item) => item.origin === 'managed_acp')).toMatchObject({
      hostLabel: 'Codex ACP',
      hostIconSvg: '<svg data-adapter="true" />',
      requestCount: 1,
      pendingCount: 1,
      canRename: true,
      canPin: true,
      canArchive: false,
      status: 'waiting',
    })
  })

  it('keeps colliding Session and request ids distinct by origin', () => {
    const projection = unified()
    const adapterSessionKey = sessionRailKey('adapter', 'codex', 'session-1')
    const acpSessionKey = sessionRailKey('managed_acp', 'codex', 'session-1')

    expect(new Set(projection.sessions.map((item) => item.key))).toEqual(new Set([
      adapterSessionKey,
      acpSessionKey,
    ]))
    expect(projection.requests.map((item) => item.rawRequestId)).toEqual([
      'feedback-1',
      'feedback-1',
    ])
    expect(new Set(projection.requests.map((item) => item.key)).size).toBe(2)
    expect(projection.requests.map((item) => item.origin).sort()).toEqual([
      'adapter',
      'managed_acp',
    ])
  })

  it('supports All Requests and stable updated-at ordering', () => {
    const projection = unified()

    expect(projection.requests.map((item) => item.origin)).toEqual([
      'adapter',
      'managed_acp',
    ])
    expect(projection.requests.map((item) => item.updatedAt)).toEqual([
      '2026-08-30T00:30:00Z',
      '2026-08-30T00:00:00Z',
    ])
    expect(unified().requests.map((item) => item.key)).toEqual(
      projection.requests.map((item) => item.key),
    )
  })

  it('filters requests by normalized text without changing their raw ids', () => {
    const projection = unified({ sessionKey: null, search: '  OLD adapter  ' })

    expect(projection.requests).toHaveLength(1)
    expect(projection.requests[0]).toMatchObject({
      origin: 'adapter',
      rawRequestId: 'feedback-1',
      id: 'feedback-1',
    })
  })

  it('uses the origin-aware Session key for Session scope', () => {
    const adapter = unified({
      sessionKey: sessionRailKey('adapter', 'codex', 'session-1'),
      search: '',
    })
    const managed = unified({
      sessionKey: sessionRailKey('managed_acp', 'codex', 'session-1'),
      search: '',
    })

    expect(adapter.requests.map((item) => item.origin)).toEqual(['adapter'])
    expect(managed.requests.map((item) => item.origin)).toEqual(['managed_acp'])
  })
})

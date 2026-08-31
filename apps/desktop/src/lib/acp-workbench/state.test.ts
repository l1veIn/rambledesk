import { describe, expect, it } from 'vitest'

import {
  isAttentionItemAnswerable,
  isCurrentPreflightContext,
  isUsablePreflight,
  itemsForSession,
  launchConfigIsComplete,
  reconcileSelection,
  resolvePreflightSelection,
  selectSession,
} from './state'
import type { AcpWorkbenchSnapshot, AttentionItem } from './types'

const items: AttentionItem[] = [
  {
    id: 'old', sessionId: 's1', kind: 'question', title: 'Answered', createdAt: '2026-08-30T10:00:00Z',
    status: 'answered', prompt: 'Done?', choices: [], multiple: false, allowSkip: true,
  },
  {
    id: 'waiting', sessionId: 's1', kind: 'permission', title: 'Allow', createdAt: '2026-08-30T09:00:00Z',
    status: 'waiting', description: 'Run command', toolTitle: 'Shell', command: 'pnpm check', path: null,
    toolCall: { kind: 'execute' },
    options: [{ id: 'allow', label: 'Allow', tone: 'allow' }],
  },
]

const snapshot: AcpWorkbenchSnapshot = {
  sessions: [
    { sessionId: 's1', title: 'One', agentId: 'codex', agentLabel: 'Codex', workspace: '/one', model: 'gpt', reasoningEffort: 'high', accessMode: 'workspace_write', status: 'waiting', pendingCount: 1, pinnedAt: null, archivedAt: null, updatedAt: '2026-08-30T10:00:00Z' },
    { sessionId: 's2', title: 'Two', agentId: 'codex', agentLabel: 'Codex', workspace: '/two', model: 'gpt', reasoningEffort: 'high', accessMode: 'workspace_write', status: 'running', pendingCount: 0, pinnedAt: null, archivedAt: null, updatedAt: '2026-08-30T11:00:00Z' },
  ],
  attentionItems: items,
  agents: [],
}

const configPreflight = {
  agentId: 'codex',
  schemaDigest: 'schema-1',
  configOptions: [
    {
      id: 'model', name: 'Model', description: null, category: 'model', source: 'agent' as const,
      kind: 'select' as const, currentValue: 'supported',
      options: [
        { value: 'supported', name: 'Supported', description: null, group: null },
        { value: 'other', name: 'Other', description: null, group: null },
      ],
    },
    {
      id: 'fast_mode', name: 'Fast mode', description: null, category: null, source: 'agent' as const,
      kind: 'boolean' as const, currentValue: false,
    },
  ],
  warning: null,
}

describe('ACP Workbench selection', () => {
  it('orders waiting attention before answered history', () => {
    expect(itemsForSession(items, 's1').map((item) => item.id)).toEqual(['waiting', 'old'])
  })

  it('preserves backend FIFO order and only allows the front live request to answer', () => {
    const queued: AttentionItem[] = [
      { ...items[1], id: 'permission-first', createdAt: '2026-08-30T09:00:00Z' },
      { ...items[1], id: 'permission-second', createdAt: '2026-08-30T11:00:00Z' },
    ]

    expect(itemsForSession(queued, 's1').map((item) => item.id)).toEqual([
      'permission-first',
      'permission-second',
    ])
    expect(isAttentionItemAnswerable(queued, 'permission-first')).toBe(true)
    expect(isAttentionItemAnswerable(queued, 'permission-second')).toBe(false)
  })

  it('reconciles missing selection to the latest session', () => {
    expect(reconcileSelection(snapshot, { sessionId: 'gone', itemId: 'gone' })).toEqual({
      sessionId: 's2', itemId: null,
    })
  })

  it('selects the first attention item for the chosen session', () => {
    expect(selectSession(snapshot, 's1')).toEqual({ sessionId: 's1', itemId: 'waiting' })
  })

  it('uses Agent defaults and only restores still-supported generic choices', () => {
    expect(resolvePreflightSelection(configPreflight, {
      model: 'stale',
      fast_mode: true,
    })).toEqual([
      { id: 'model', value: 'supported' },
      { id: 'fast_mode', value: true },
    ])
    expect(launchConfigIsComplete(configPreflight, [
      { id: 'model', value: 'supported' },
      { id: 'fast_mode', value: true },
    ])).toBe(true)
  })

  it('does not present an empty failed preflight as ready', () => {
    expect(isUsablePreflight(null)).toBe(false)
    expect(isUsablePreflight({
      agentId: 'codex',
      schemaDigest: '',
      configOptions: [],
      warning: 'Agent could not start',
    })).toBe(false)
    expect(isUsablePreflight({
      agentId: 'agent-with-defaults',
      schemaDigest: 'schema-defaults',
      configOptions: [],
      warning: null,
    })).toBe(true)
    expect(isUsablePreflight({
      agentId: 'codex',
      schemaDigest: 'schema-1',
      configOptions: configPreflight.configOptions,
      warning: null,
    })).toBe(true)
    expect(isUsablePreflight({
      agentId: 'future-agent',
      schemaDigest: 'schema-2',
      configOptions: [{
        id: 'future', name: 'Future', description: null, category: null, source: 'agent',
        kind: 'unsupported', currentValue: null, rawType: 'number',
      }],
      warning: 'Unsupported option',
    })).toBe(true)
  })

  it('rejects a preflight response after workspace, Agent, or generation changes', () => {
    const expected = { generation: 3, workspace: '/one', agentId: 'codex' }

    expect(isCurrentPreflightContext(expected, expected)).toBe(true)
    expect(isCurrentPreflightContext(expected, { ...expected, workspace: '/two' })).toBe(false)
    expect(isCurrentPreflightContext(expected, { ...expected, agentId: 'other' })).toBe(false)
    expect(isCurrentPreflightContext(expected, { ...expected, generation: 4 })).toBe(false)
  })
})

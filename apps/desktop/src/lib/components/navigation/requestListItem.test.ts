import { describe, expect, it } from 'vitest'

import {
  requestListItemKindClass,
  requestListItemKindLabel,
  requestListItemStatusClass,
  workbenchRequestKey,
  type WorkbenchRequestListItem,
} from './requestListItem'

const shared = {
  key: workbenchRequestKey('managed_acp', 'managed-session', 'request-1'),
  origin: 'managed_acp',
  rawRequestId: 'request-1',
  sessionKey: 'managed-session',
  status: 'waiting',
  sessionId: 'session-1',
  agentId: 'codex',
  sourceHint: '/workspace/rambledesk',
  createdAt: '2026-08-30T01:00:00.000Z',
  updatedAt: '2026-08-30T01:00:00.000Z',
} as const

describe('request list item presentation', () => {
  it.each([
    ['feedback', 'Ramble Feedback', 'text-primary'],
    ['permission', 'Permission', 'text-warning'],
    ['question', 'Ask Question', 'text-info'],
  ] as const)('presents %s requests distinctly', (kind, label, className) => {
    const item: WorkbenchRequestListItem = {
      ...shared,
      id: `${kind}-1`,
      kind,
      title: label,
      summary: `${label} summary`,
    }

    expect(item.kind).toBe(kind)
    expect(requestListItemKindLabel(item.kind)).toBe(label)
    expect(requestListItemKindClass(item.kind)).toContain(className)
  })

  it.each([
    ['waiting', 'bg-warning'],
    ['in_progress', 'bg-info'],
    ['completed', 'bg-success'],
    ['cancelled', 'bg-destructive'],
    ['cooking', 'bg-primary'],
  ] as const)('maps %s to its existing status treatment', (status, className) => {
    expect(requestListItemStatusClass(status)).toContain(className)
  })

  it('keeps identical wire ids distinct across origins and Sessions', () => {
    expect(workbenchRequestKey('adapter', 'legacy-session', 'same')).not.toBe(
      workbenchRequestKey('managed_acp', 'managed-session', 'same'),
    )
  })
})

import { describe, expect, it } from 'vitest'

import {
  orderSessionRailItems,
  sessionRailActions,
  sessionRailKey,
  sessionRailStatusPresentation,
  sessionRailTotals,
  type SessionRailItem,
} from './sessionRailItem'

function item(overrides: Partial<SessionRailItem> = {}): SessionRailItem {
  return {
    key: sessionRailKey('adapter', 'codex', 'session-1'),
    origin: 'adapter',
    hostId: 'codex',
    sessionId: 'session-1',
    title: 'Session one',
    hostLabel: 'Codex',
    hostIconSvg: '<svg></svg>',
    requestCount: 2,
    pendingCount: 1,
    updatedAt: '2026-08-30T10:00:00Z',
    pinnedAt: null,
    status: 'waiting',
    canRename: true,
    canPin: true,
    canArchive: true,
    ...overrides,
  }
}

describe('session rail presentation helpers', () => {
  it('builds a collision-resistant key across origins, hosts, and sessions', () => {
    expect(sessionRailKey('managed_acp', 'codex', 'same')).toBe(
      'managed_acp\u0000codex\u0000same',
    )
    expect(sessionRailKey('adapter', 'codex', 'same')).not.toBe(
      sessionRailKey('managed_acp', 'codex', 'same'),
    )
  })

  it('orders pinned sessions first, then most recently updated without mutating input', () => {
    const older = item({ key: 'older', sessionId: 'older', updatedAt: '2026-08-29T10:00:00Z' })
    const newer = item({ key: 'newer', sessionId: 'newer', updatedAt: '2026-08-31T10:00:00Z' })
    const pinned = item({
      key: 'pinned',
      sessionId: 'pinned',
      pinnedAt: '2026-08-28T10:00:00Z',
      updatedAt: '2026-08-28T10:00:00Z',
    })
    const input = [older, pinned, newer]

    expect(orderSessionRailItems(input).map((candidate) => candidate.key)).toEqual([
      'pinned',
      'newer',
      'older',
    ])
    expect(input.map((candidate) => candidate.key)).toEqual(['older', 'pinned', 'newer'])
  })

  it('projects all-request counts across legacy and ACP sessions', () => {
    expect(sessionRailTotals([
      item(),
      item({
        key: 'acp',
        origin: 'managed_acp',
        requestCount: 4,
        pendingCount: 2,
      }),
    ])).toEqual({ requests: 6, pending: 3 })
  })

  it('exposes only actions backed by item capabilities', () => {
    expect(sessionRailActions(item({ canRename: false, canPin: false, canArchive: false }))).toEqual({
      rename: false,
      pin: false,
      archive: false,
      any: false,
    })
    expect(sessionRailActions(item({ canRename: false, canPin: true, canArchive: false }))).toEqual({
      rename: false,
      pin: true,
      archive: false,
      any: true,
    })
  })

  it('shows a spinner only while the Agent is actively running', () => {
    expect(sessionRailStatusPresentation('running')).toEqual({
      kind: 'running',
      label: 'Running',
      spinning: true,
    })
    expect(sessionRailStatusPresentation('offline')).toMatchObject({
      kind: 'offline',
      spinning: false,
    })
    expect(sessionRailStatusPresentation('error')).toMatchObject({
      kind: 'error',
      spinning: false,
    })
  })

  it('prioritizes every waiting state over a generic running indicator', () => {
    expect(sessionRailStatusPresentation('waiting')).toEqual({
      kind: 'waiting',
      label: 'Waiting for you',
      spinning: false,
    })
    expect(sessionRailStatusPresentation('waiting_feedback')).toMatchObject({
      kind: 'waiting',
      label: 'Ramble Feedback',
      spinning: false,
    })
    expect(sessionRailStatusPresentation('waiting-permission')).toMatchObject({
      kind: 'waiting',
      label: 'Permission Request',
      spinning: false,
    })
    expect(sessionRailStatusPresentation('waiting_question')).toMatchObject({
      kind: 'waiting',
      label: 'Ask Question',
      spinning: false,
    })
    expect(sessionRailStatusPresentation('waiting-running')).toMatchObject({
      kind: 'waiting',
      spinning: false,
    })
  })

  it('does not render runtime state for legacy Sessions or unknown values', () => {
    expect(sessionRailStatusPresentation(null)).toBeNull()
    expect(sessionRailStatusPresentation('unexpected')).toBeNull()
  })
})

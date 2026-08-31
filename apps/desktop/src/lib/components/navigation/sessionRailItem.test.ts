import { describe, expect, it } from 'vitest'

import {
  orderSessionRailItems,
  sessionRailActions,
  sessionRailKey,
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
})

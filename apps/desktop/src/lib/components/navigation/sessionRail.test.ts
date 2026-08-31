import { describe, expect, it } from 'vitest'

import type { HostSessionSummary } from '$lib/feedback'
import { hostSessionKey, orderSessionRailSessions } from './sessionRail'

function session(
  hostId: string,
  hostSessionId: string,
  updatedAt: string,
  pinnedAt: string | null = null,
  hostPinnedAt: string | null = null,
): HostSessionSummary {
  return {
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

  it('keeps every session from a pinned host visible at the front of the flat list', () => {
    const sessions = [
      session('pi', 'pinned-session', '2026-09-01T10:00:00Z', '2026-09-01T11:00:00Z'),
      session('codex', 'older-host-session', '2026-08-29T10:00:00Z', null, '2026-09-01T12:00:00Z'),
      session('codex', 'newer-host-session', '2026-08-30T10:00:00Z', null, '2026-09-01T12:00:00Z'),
    ]

    expect(orderSessionRailSessions(sessions).map(hostSessionKey)).toEqual([
      'session:["codex","newer-host-session"]',
      'session:["codex","older-host-session"]',
      'session:["pi","pinned-session"]',
    ])
  })
})

import { describe, expect, it } from 'vitest'

import { workspaceTabNavigationTarget } from './workspaceTabNavigation'

describe('workspaceTabNavigationTarget', () => {
  const keys = [
    'session:["codex","alpha"]',
    'session:["codex","beta"]',
    'session:["pi","alpha"]',
  ]

  it('moves to adjacent tabs and wraps at both ends', () => {
    expect(workspaceTabNavigationTarget(keys, keys[0], 'next')).toBe(keys[1])
    expect(workspaceTabNavigationTarget(keys, keys[2], 'next')).toBe(keys[0])
    expect(workspaceTabNavigationTarget(keys, keys[2], 'previous')).toBe(keys[1])
    expect(workspaceTabNavigationTarget(keys, keys[0], 'previous')).toBe(keys[2])
  })

  it('moves directly to the first or last tab', () => {
    expect(workspaceTabNavigationTarget(keys, keys[1], 'first')).toBe(keys[0])
    expect(workspaceTabNavigationTarget(keys, keys[1], 'last')).toBe(keys[2])
  })

  it('uses a deterministic edge when focus is absent or stale', () => {
    expect(workspaceTabNavigationTarget(keys, null, 'next')).toBe(keys[0])
    expect(workspaceTabNavigationTarget(keys, 'session:missing', 'previous')).toBe(keys[2])
  })

  it('handles empty and single-tab workspaces', () => {
    expect(workspaceTabNavigationTarget([], null, 'next')).toBeNull()
    expect(workspaceTabNavigationTarget([keys[0]], keys[0], 'next')).toBe(keys[0])
    expect(workspaceTabNavigationTarget([keys[0]], keys[0], 'previous')).toBe(keys[0])
  })
})

import { describe, expect, it } from 'vitest'

import {
  requestWorkspaceTabActivation,
  workspaceTabId,
  workspaceTabKeyboardAction,
  workspaceTabNavigationTarget,
  workspaceTabPanelId,
} from './workspaceTabNavigation'

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

  it('maps keyboard input to focus, activation, and close actions', () => {
    expect(workspaceTabKeyboardAction('ArrowLeft')).toEqual({ type: 'move', intent: 'previous' })
    expect(workspaceTabKeyboardAction('Home')).toEqual({ type: 'move', intent: 'first' })
    expect(workspaceTabKeyboardAction('Enter')).toEqual({ type: 'activate' })
    expect(workspaceTabKeyboardAction(' ')).toEqual({ type: 'activate' })
    expect(workspaceTabKeyboardAction('Delete')).toEqual({ type: 'close' })
    expect(workspaceTabKeyboardAction('Escape')).toBeNull()
  })

  it('derives stable and distinct tab and panel ids from the composite view key', () => {
    expect(workspaceTabId(keys[0])).toBe(
      'workspace-tab-session%3A%5B%22codex%22%2C%22alpha%22%5D',
    )
    expect(workspaceTabPanelId(keys[0])).toBe(
      'workspace-tabpanel-session%3A%5B%22codex%22%2C%22alpha%22%5D',
    )
    expect(workspaceTabId(keys[0])).not.toBe(workspaceTabId(keys[2]))
    expect(workspaceTabId(keys[0])).not.toBe(workspaceTabPanelId(keys[0]))
  })

  it('uses the same explicit activation contract for pointer click and keyboard activation', () => {
    const activations: string[] = []
    const activate = (viewKey: string) => activations.push(viewKey)

    expect(requestWorkspaceTabActivation(keys[1], false, activate)).toBe(true)
    expect(requestWorkspaceTabActivation(keys[2], true, activate)).toBe(false)
    expect(activations).toEqual([keys[1]])
  })
})

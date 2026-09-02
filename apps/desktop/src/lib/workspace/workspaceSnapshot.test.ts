import { describe, expect, it } from 'vitest'

import {
  inboxViewDescriptor,
  rambelleProfileViewDescriptor,
  requestTaskViewDescriptor,
  sessionViewDescriptor,
  settingsViewDescriptor,
  workspaceViewKey,
} from './viewDescriptors'
import { workspaceShellReducer, EMPTY_WORKSPACE_SHELL_STATE } from './workspaceShell'
import {
  createWorkspaceSnapshot,
  MAX_WORKSPACE_SNAPSHOT_VIEWS,
  restoreWorkspaceSnapshot,
} from './workspaceSnapshot'

const alpha = sessionViewDescriptor('codex', 'alpha')
const beta = sessionViewDescriptor('pi', 'beta')

describe('workspace snapshots', () => {
  it('round-trips view order, active identity, and request hints', () => {
    const openedAlpha = workspaceShellReducer(EMPTY_WORKSPACE_SHELL_STATE, {
      type: 'open',
      view: alpha,
    })
    const state = workspaceShellReducer(openedAlpha, { type: 'open', view: beta })
    const requestIds = new Map([
      [workspaceViewKey(alpha), 'request-alpha'],
      [workspaceViewKey(beta), 'request-beta'],
    ])

    const serialized = createWorkspaceSnapshot(state, requestIds)
    const restored = restoreWorkspaceSnapshot(serialized)

    expect(serialized).toEqual({
      version: 2,
      views: [
        { ...alpha, lastRequestId: 'request-alpha' },
        { ...beta, lastRequestId: 'request-beta' },
      ],
      activeViewKey: workspaceViewKey(beta),
    })
    expect(restored?.shellState).toEqual(state)
    expect([...restored!.requestIds]).toEqual([...requestIds])
  })

  it('round-trips a reordered tab strip without changing its active identity', () => {
    const opened = workspaceShellReducer(
      workspaceShellReducer(EMPTY_WORKSPACE_SHELL_STATE, { type: 'open', view: alpha }),
      { type: 'open', view: beta },
    )
    const reordered = workspaceShellReducer(opened, {
      type: 'reorder',
      viewKeys: [workspaceViewKey(beta), workspaceViewKey(alpha)],
    })

    const restored = restoreWorkspaceSnapshot(
      createWorkspaceSnapshot(reordered, new Map()),
    )

    expect(restored?.shellState.views).toEqual([beta, alpha])
    expect(restored?.shellState.activeViewKey).toBe(workspaceViewKey(beta))
  })

  it('reads v1 snapshots while keeping cross-host sessions distinct and deduplicated', () => {
    const value = {
      version: 1,
      views: [
        { kind: 'session', hostId: 'codex', hostSessionId: 'shared', lastRequestId: 'first' },
        { kind: 'session', hostId: 'pi', hostSessionId: 'shared', lastRequestId: 'second' },
        { kind: 'session', hostId: 'codex', hostSessionId: 'shared', lastRequestId: 'duplicate' },
      ],
      activeViewKey: 'untrusted-key',
    }

    const restored = restoreWorkspaceSnapshot(value)

    expect(restored?.shellState.views).toEqual([
      sessionViewDescriptor('codex', 'shared'),
      sessionViewDescriptor('pi', 'shared'),
    ])
    expect(restored?.shellState.activeViewKey).toBe(
      workspaceViewKey(sessionViewDescriptor('codex', 'shared')),
    )
    expect(restored?.requestIds.get(workspaceViewKey(sessionViewDescriptor('codex', 'shared')))).toBe(
      'first',
    )
  })

  it('drops malformed entries, optional invalid request hints, and caps restored views', () => {
    const views = Array.from({ length: MAX_WORKSPACE_SNAPSHOT_VIEWS + 5 }, (_, index) => ({
      kind: 'session',
      hostId: 'codex',
      hostSessionId: `session-${index}`,
      lastRequestId: index === 0 ? 'x'.repeat(513) : `request-${index}`,
    }))
    views.splice(1, 0, {
      kind: 'session',
      hostId: '',
      hostSessionId: 'invalid',
      lastRequestId: 'request-invalid',
    })

    const restored = restoreWorkspaceSnapshot({ version: 2, views, activeViewKey: null })

    expect(restored?.shellState.views).toHaveLength(MAX_WORKSPACE_SNAPSHOT_VIEWS)
    expect(restored?.requestIds.has(workspaceViewKey(sessionViewDescriptor('codex', 'session-0')))).toBe(
      false,
    )
  })

  it('preserves a valid empty snapshot and rejects corrupt or unsupported snapshots', () => {
    expect(restoreWorkspaceSnapshot({ version: 2, views: [], activeViewKey: null })).toEqual({
      shellState: EMPTY_WORKSPACE_SHELL_STATE,
      requestIds: new Map(),
    })
    expect(restoreWorkspaceSnapshot({ version: 99, views: [], activeViewKey: null })).toBeNull()
    expect(restoreWorkspaceSnapshot({ version: 2, views: 'invalid', activeViewKey: null })).toBeNull()
    expect(restoreWorkspaceSnapshot(null)).toBeNull()
  })

  it('serializes only client navigation fields', () => {
    const state = workspaceShellReducer(EMPTY_WORKSPACE_SHELL_STATE, {
      type: 'open',
      view: alpha,
    })
    const serialized = JSON.stringify(
      createWorkspaceSnapshot(state, new Map([[workspaceViewKey(alpha), 'request-alpha']])),
    )

    expect(serialized).not.toContain('document')
    expect(serialized).not.toContain('markdown')
    expect(serialized).not.toContain('revision')
    expect(serialized).not.toContain('attachment')
    expect(serialized).not.toContain('runtime')
  })

  it('persists settings identity without transient section or session request fields', () => {
    const sessionState = workspaceShellReducer(EMPTY_WORKSPACE_SHELL_STATE, {
      type: 'open',
      view: alpha,
    })
    const settingsState = workspaceShellReducer(sessionState, {
      type: 'open',
      view: settingsViewDescriptor(),
    })

    expect(createWorkspaceSnapshot(settingsState, new Map())).toEqual({
      version: 2,
      views: [{ ...alpha, lastRequestId: null }, { kind: 'settings' }],
      activeViewKey: workspaceViewKey(settingsViewDescriptor()),
    })
    expect(
      restoreWorkspaceSnapshot({
        version: 2,
        views: [{ kind: 'settings' }, { ...alpha, lastRequestId: null }],
        activeViewKey: 'settings:singleton',
      })?.shellState,
    ).toEqual({
      views: [settingsViewDescriptor(), alpha],
      activeViewKey: workspaceViewKey(settingsViewDescriptor()),
    })
  })

  it('round-trips the aggregate Inbox as a singleton client view', () => {
    const state = workspaceShellReducer(EMPTY_WORKSPACE_SHELL_STATE, {
      type: 'open',
      view: inboxViewDescriptor(),
    })
    const snapshot = createWorkspaceSnapshot(state, new Map())

    expect(snapshot).toEqual({
      version: 2,
      views: [{ kind: 'inbox' }],
      activeViewKey: 'inbox:singleton',
    })
    expect(restoreWorkspaceSnapshot(snapshot)?.shellState).toEqual(state)
  })

  it('round-trips request task and profile descriptors without editor state', () => {
    const task = requestTaskViewDescriptor('request-alpha')
    const taskState = workspaceShellReducer(EMPTY_WORKSPACE_SHELL_STATE, {
      type: 'open',
      view: task,
    })
    const state = workspaceShellReducer(taskState, {
      type: 'open',
      view: rambelleProfileViewDescriptor(),
    })
    const snapshot = createWorkspaceSnapshot(state, new Map())

    expect(snapshot).toEqual({
      version: 2,
      views: [
        { kind: 'request-task', requestId: 'request-alpha' },
        { kind: 'rambelle-profile' },
      ],
      activeViewKey: 'rambelle-profile:singleton',
    })
    expect(restoreWorkspaceSnapshot(snapshot)?.shellState).toEqual(state)
    expect(JSON.stringify(snapshot)).not.toContain('draft')
  })
})

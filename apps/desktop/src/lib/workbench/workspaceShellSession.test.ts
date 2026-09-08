import { get } from 'svelte/store'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { workspaceShellReducer, EMPTY_WORKSPACE_SHELL_STATE } from '../workspace/workspaceShell'
import { requestTaskViewDescriptor, sessionViewDescriptor, workspaceViewKey } from '../workspace/viewDescriptors'

const storage = new Map<string, string>()

vi.mock('../uiPreferences', () => ({
  saveWorkspaceSnapshot: (snapshot: unknown) => storage.set('ui', JSON.stringify(snapshot)),
  savedWorkspaceSnapshot: () => null,
}))
vi.mock('../workspace/previewWorkspaceSnapshot', () => ({
  savePreviewWorkspaceSnapshot: () => {},
  savedPreviewWorkspaceSnapshot: () => null,
}))

const { createWorkspaceShellSession } = await import('./workspaceShellSession')

const sessionView = sessionViewDescriptor('codex', 'alpha')
const taskView = requestTaskViewDescriptor('request-1')

describe('workspace shell session', () => {
  beforeEach(() => {
    storage.clear()
  })

  it('opens, activates and closes views through the reducer', () => {
    const shell = createWorkspaceShellSession({ previewMode: false })

    expect(shell.dispatch({ type: 'open', view: sessionView })).toBe(true)
    expect(shell.dispatch({ type: 'open', view: sessionView })).toBe(false)
    expect(shell.dispatch({ type: 'open', view: taskView })).toBe(true)
    expect(shell.activeView()).toEqual(taskView)
    expect(get(shell).shell.views).toEqual([sessionView, taskView])

    shell.dispatch({ type: 'focus', viewKey: workspaceViewKey(sessionView) })
    expect(shell.activeViewKey()).toBe(workspaceViewKey(sessionView))

    shell.dispatch({ type: 'close', viewKey: workspaceViewKey(sessionView) })
    expect(get(shell).shell.views).toEqual([taskView])
    expect(shell.activeView()).toEqual(taskView)
  })

  it('persists after every open, close and reorder', () => {
    const shell = createWorkspaceShellSession({ previewMode: false })
    shell.dispatch({ type: 'open', view: sessionView })
    shell.dispatch({ type: 'open', view: taskView })
    shell.dispatch({
      type: 'reorder',
      viewKeys: [workspaceViewKey(taskView), workspaceViewKey(sessionView)],
    })

    expect(JSON.parse(storage.get('ui')!)).toMatchObject({
      views: [taskView, sessionView],
      activeViewKey: workspaceViewKey(taskView),
    })
  })

  it('remembers and forgets the request shown by a session view', () => {
    const shell = createWorkspaceShellSession({ previewMode: false })
    shell.dispatch({ type: 'open', view: sessionView })

    shell.bindRequest(workspaceViewKey(sessionView), 'request-1')
    expect(shell.requestIdFor(sessionView)).toBe('request-1')

    shell.forgetRequest(workspaceViewKey(sessionView))
    expect(shell.requestIdFor(sessionView)).toBeUndefined()
  })

  it('tracks the pending activation target', () => {
    const shell = createWorkspaceShellSession({ previewMode: false })
    expect(shell.pendingViewKey()).toBeNull()
    shell.setPendingViewKey(workspaceViewKey(taskView))
    expect(shell.pendingViewKey()).toBe(workspaceViewKey(taskView))
    expect(get(shell).pendingViewKey).toBe(workspaceViewKey(taskView))
  })

  it('reports no restored view when nothing was saved', () => {
    const shell = createWorkspaceShellSession({ previewMode: false })
    expect(shell.restoredActiveView()).toBe(false)
    expect(get(shell)).toEqual({
      shell: EMPTY_WORKSPACE_SHELL_STATE,
      requestIds: new Map(),
      pendingViewKey: null,
    })
    expect(workspaceShellReducer(EMPTY_WORKSPACE_SHELL_STATE, { type: 'focus', viewKey: 'x' })).toBe(
      EMPTY_WORKSPACE_SHELL_STATE,
    )
  })
})

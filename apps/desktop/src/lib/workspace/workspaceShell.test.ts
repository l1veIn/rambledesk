import { describe, expect, it } from 'vitest'

import {
  sessionViewDescriptor,
  workspaceViewKey,
  type WorkspaceViewDescriptor,
} from './viewDescriptors'
import {
  activeWorkspaceView,
  EMPTY_WORKSPACE_SHELL_STATE,
  workspaceShellReducer,
  type WorkspaceShellAction,
  type WorkspaceShellState,
} from './workspaceShell'

const alpha = sessionViewDescriptor('codex', 'alpha')
const beta = sessionViewDescriptor('codex', 'beta')
const gamma = sessionViewDescriptor('pi', 'gamma')

function reduce(
  state: WorkspaceShellState,
  ...actions: WorkspaceShellAction[]
): WorkspaceShellState {
  return actions.reduce(workspaceShellReducer, state)
}

function open(view: WorkspaceViewDescriptor): WorkspaceShellAction {
  return { type: 'open', view }
}

function expectValidState(state: WorkspaceShellState) {
  const keys = state.views.map(workspaceViewKey)
  expect(new Set(keys).size).toBe(keys.length)
  if (state.activeViewKey === null) expect(state.views).toHaveLength(0)
  else expect(keys).toContain(state.activeViewKey)
}

describe('workspaceShellReducer', () => {
  it('starts empty and opens consecutive views with the latest active', () => {
    expect(EMPTY_WORKSPACE_SHELL_STATE).toEqual({ views: [], activeViewKey: null })
    expect(activeWorkspaceView(EMPTY_WORKSPACE_SHELL_STATE)).toBeNull()

    const first = workspaceShellReducer(EMPTY_WORKSPACE_SHELL_STATE, open(alpha))
    const second = workspaceShellReducer(first, open(beta))

    expect(first.views).toEqual([alpha])
    expect(activeWorkspaceView(first)).toBe(alpha)
    expect(second.views).toEqual([alpha, beta])
    expect(activeWorkspaceView(second)).toBe(beta)
    expectValidState(first)
    expectValidState(second)
  })

  it('deduplicates by stable key, focuses the existing view, and keeps its position', () => {
    const duplicateAlpha = sessionViewDescriptor('codex', 'alpha')
    const beforeDuplicate = reduce(
      EMPTY_WORKSPACE_SHELL_STATE,
      open(alpha),
      open(beta),
    )
    const result = workspaceShellReducer(beforeDuplicate, open(duplicateAlpha))

    expect(result.views).toEqual([alpha, beta])
    expect(result.views[0]).toBe(alpha)
    expect(result.activeViewKey).toBe(workspaceViewKey(alpha))
    expectValidState(result)
  })

  it('keeps the same session id from different hosts as distinct views', () => {
    const codex = sessionViewDescriptor('codex', 'shared')
    const pi = sessionViewDescriptor('pi', 'shared')
    const result = reduce(EMPTY_WORKSPACE_SHELL_STATE, open(codex), open(pi))

    expect(result.views).toEqual([codex, pi])
    expect(activeWorkspaceView(result)).toBe(pi)
    expectValidState(result)
  })

  it('focuses known views and ignores active or unknown keys', () => {
    const initial = reduce(EMPTY_WORKSPACE_SHELL_STATE, open(alpha), open(beta))
    const focused = workspaceShellReducer(initial, {
      type: 'focus',
      viewKey: workspaceViewKey(alpha),
    })

    expect(activeWorkspaceView(focused)).toBe(alpha)
    expect(
      workspaceShellReducer(focused, {
        type: 'focus',
        viewKey: workspaceViewKey(alpha),
      }),
    ).toBe(focused)
    expect(workspaceShellReducer(focused, { type: 'focus', viewKey: 'session:missing' })).toBe(
      focused,
    )
    expectValidState(focused)
  })

  it('closes an inactive view without changing the active view', () => {
    const initial = reduce(
      EMPTY_WORKSPACE_SHELL_STATE,
      open(alpha),
      open(beta),
      open(gamma),
    )
    const result = workspaceShellReducer(initial, {
      type: 'close',
      viewKey: workspaceViewKey(alpha),
    })

    expect(result.views).toEqual([beta, gamma])
    expect(activeWorkspaceView(result)).toBe(gamma)
    expectValidState(result)
  })

  it('focuses the right neighbor, otherwise the left neighbor, when closing active views', () => {
    const middleActive = reduce(
      EMPTY_WORKSPACE_SHELL_STATE,
      open(alpha),
      open(beta),
      open(gamma),
      { type: 'focus', viewKey: workspaceViewKey(beta) },
    )
    const closedMiddle = workspaceShellReducer(middleActive, {
      type: 'close',
      viewKey: workspaceViewKey(beta),
    })
    expect(closedMiddle.views).toEqual([alpha, gamma])
    expect(activeWorkspaceView(closedMiddle)).toBe(gamma)

    const closedRight = workspaceShellReducer(
      reduce(EMPTY_WORKSPACE_SHELL_STATE, open(alpha), open(beta), open(gamma)),
      { type: 'close', viewKey: workspaceViewKey(gamma) },
    )
    expect(closedRight.views).toEqual([alpha, beta])
    expect(activeWorkspaceView(closedRight)).toBe(beta)

    const closedLeft = workspaceShellReducer(
      reduce(
        EMPTY_WORKSPACE_SHELL_STATE,
        open(alpha),
        open(beta),
        open(gamma),
        { type: 'focus', viewKey: workspaceViewKey(alpha) },
      ),
      { type: 'close', viewKey: workspaceViewKey(alpha) },
    )
    expect(closedLeft.views).toEqual([beta, gamma])
    expect(activeWorkspaceView(closedLeft)).toBe(beta)
    expectValidState(closedMiddle)
    expectValidState(closedRight)
    expectValidState(closedLeft)
  })

  it('becomes empty after closing the last active view and ignores unknown closes', () => {
    const initial = workspaceShellReducer(EMPTY_WORKSPACE_SHELL_STATE, open(alpha))
    const unknownClose = workspaceShellReducer(initial, {
      type: 'close',
      viewKey: 'session:missing',
    })
    const result = workspaceShellReducer(initial, {
      type: 'close',
      viewKey: workspaceViewKey(alpha),
    })

    expect(unknownClose).toBe(initial)
    expect(result).toEqual(EMPTY_WORKSPACE_SHELL_STATE)
    expect(activeWorkspaceView(result)).toBeNull()
    expectValidState(result)
  })

  it('does not mutate prior state or descriptor arrays', () => {
    const views = Object.freeze([alpha, beta])
    const initial: WorkspaceShellState = Object.freeze({
      views,
      activeViewKey: workspaceViewKey(beta),
    })
    const snapshot = [...views]

    const result = workspaceShellReducer(initial, open(gamma))

    expect(initial.views).toBe(views)
    expect(initial.views).toEqual(snapshot)
    expect(result.views).toEqual([alpha, beta, gamma])
    expect(result.views).not.toBe(initial.views)
    expectValidState(initial)
    expectValidState(result)
  })
})

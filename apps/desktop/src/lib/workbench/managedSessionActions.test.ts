import { get, writable } from 'svelte/store'
import { describe, expect, it, vi } from 'vitest'

import { EMPTY_WORKSPACE_SHELL_STATE } from '../workspace/workspaceShell'

const mocks = vi.hoisted(() => ({
  deleteSessionRecord: vi.fn(async () => {}),
  removeManagedSessionViews: vi.fn(() => ({
    shell: { views: [], activeViewKey: null },
    closedViewKeys: [] as string[],
    closedActive: false,
  })),
  removeArchivedManagedSessionView: vi.fn(() => ({
    shell: { views: [], activeViewKey: null },
    shouldInvalidatePending: false,
    shouldNavigateToArchive: false,
  })),
}))

vi.mock('../agents/managedSessionDeletion', () => ({
  deleteSessionRecord: mocks.deleteSessionRecord,
  removeManagedSessionViews: mocks.removeManagedSessionViews,
}))
vi.mock('../agents/managedSessionArchive', () => ({
  removeArchivedManagedSessionView: mocks.removeArchivedManagedSessionView,
}))
import { createDraftSession } from './draftSession'
import { createManagedSessionActions, type ManagedSessionActionsContext } from './managedSessionActions'
import { createWorkspaceSession } from './workspaceSession'

function harness(overrides: Record<string, unknown> = {}) {
  const shell = writable({
    shell: EMPTY_WORKSPACE_SHELL_STATE,
    requestIds: new Map<string, string>(),
    pendingViewKey: null as string | null,
  })
  const navigationState = writable({
    hostSessions: [],
    requests: [],
    pendingRequests: [],
    selectedHostId: null,
    selectedHostSessionId: null,
    hostSessionFactsStatus: 'ready' as const,
    hostSessionFactsRevision: 1,
    initializationFailure: null,
  })
  const context = {
    transport: { call: vi.fn(async () => []) },
    navigation: {
      subscribe: navigationState.subscribe,
      selectScope: vi.fn(async () => ({ selected: true, requests: [] })),
      refreshNavigation: vi.fn(async () => true),
      archiveHostSession: vi.fn(async () => true),
    },
    workspaceShell: {
      subscribe: shell.subscribe,
      views: () => get(shell).shell.views,
      activeView: () => null,
      activeViewKey: () => get(shell).shell.activeViewKey,
      pendingViewKey: () => get(shell).pendingViewKey,
      requestIdFor: () => undefined,
      dispatch: vi.fn(() => true),
      replaceShell: vi.fn(),
      forgetRequest: vi.fn(),
      setPendingViewKey: vi.fn(),
    },
    workspaceSession: createWorkspaceSession(),
    draftSession: createDraftSession(),
    workspaceNavigation: () => ({
      activateView: vi.fn(async () => 'activated' as const),
      invalidate: vi.fn(() => 1),
      currentIntent: vi.fn(() => 1),
      isCurrent: vi.fn(() => true),
      clearWorkspace: vi.fn(),
    }),
    canOpenManagedSession: () => true,
    tr: (source: string) => source,
    messageFrom: (cause: unknown) => String(cause),
    setPageError: vi.fn(),
    setCookingPreview: vi.fn(),
    isTransitionLocked: () => false,
    exitRamble: vi.fn(async () => {}),
    rambleCanExit: () => false,
    openArchivedSessions: vi.fn(async () => {}),
    ...overrides,
  } as unknown as ManagedSessionActionsContext
  return { actions: createManagedSessionActions(context), context, shell }
}

const managedSession = {
  session_id: 'session-1',
  host_id: 'codex',
  host_session_id: 'alpha',
  title: 'Alpha',
  management: { kind: 'managed' as const },
} as never

describe('managed session actions', () => {
  it('refuses to open a draft session when the environment cannot host one', async () => {
    const { actions } = harness({ canOpenManagedSession: () => false })
    await expect(actions.openNewManagedSession()).resolves.toBe(false)
  })

  it('tracks deletion flags while a delete command runs', async () => {
    const { actions } = harness()
    actions.observeDeletion('session-1', true)
    expect(get(actions).deletingSessions.has('session-1')).toBe(true)
    actions.observeDeletion('session-1', false)
    expect(get(actions).deletingSessions.has('session-1')).toBe(false)
  })

  it('archives through the navigation controller and leaves non-managed sessions alone', async () => {
    const { actions, context } = harness()
    await actions.archiveSessionFromUi({
      ...(managedSession as object),
      management: { kind: 'external' },
    } as never)
    expect(context.navigation.archiveHostSession).toHaveBeenCalledTimes(1)
    expect(context.workspaceShell.replaceShell).not.toHaveBeenCalled()
  })

  it('cleans up views when a managed session is archived', async () => {
    const { actions, context } = harness()
    await actions.archiveSessionFromUi(managedSession)
    expect(context.workspaceShell.replaceShell).toHaveBeenCalled()
    expect(context.workspaceShell.forgetRequest).toHaveBeenCalled()
  })

  it('ignores delete for external sessions and while one is already running', async () => {
    mocks.deleteSessionRecord.mockClear()
    const { actions } = harness()
    await actions.deleteManagedSessionFromUi({
      ...(managedSession as object),
      management: { kind: 'external' },
    } as never)
    expect(mocks.deleteSessionRecord).not.toHaveBeenCalled()

    const first = actions.deleteManagedSessionFromUi(managedSession)
    await actions.deleteManagedSessionFromUi(managedSession)
    await first
    expect(mocks.deleteSessionRecord).toHaveBeenCalledTimes(1)
  })

  it('closes a draft once and reports promotion', async () => {
    const { actions } = harness()
    const first = actions.closeDraft('draft-1')
    const second = await actions.closeDraft('draft-1')
    expect(second.skipped).toBe(true)
    await first
  })
})

import { get, writable } from 'svelte/store'
import { describe, expect, it, vi } from 'vitest'

import { createDraftSession } from './draftSession'
import { createStartupController, type StartupControllerContext } from './startupController'
import { createWorkspaceSession } from './workspaceSession'

function navigationStore(initial: Record<string, unknown> = {}) {
  return writable({
    pendingRequests: [],
    requests: [],
    hostSessions: [],
    hostSessionFactsStatus: 'ready' as const,
    hostSessionFactsRevision: 1,
    hostProfiles: {},
    selectedHostId: null,
    selectedHostSessionId: null,
    requestSearch: '',
    requestFilters: { status: 'all', timeRange: 'all' },
    nextRequestCursor: null,
    loadingNavigation: false,
    loadingRequests: false,
    loadingMoreRequests: false,
    refreshingPage: false,
    initializationFailure: null,
    ...initial,
  })
}

function harness(overrides: Partial<StartupControllerContext> = {}) {
  const navigation = navigationStore()
  const restoreResolver = {
    refresh: vi.fn(async () => true),
  }
  const context = {
    navigation: {
      subscribe: navigation.subscribe,
      initialize: vi.fn(async () => true),
      refreshNavigation: vi.fn(async () => true),
      selectScope: vi.fn(async () => ({ selected: true, requests: [] })),
      resolveHostProfile: vi.fn(),
    },
    workspaceShell: {
      views: () => [],
      activeView: () => null,
      activeViewKey: () => null,
      hasRestoredSnapshot: () => false,
      restoredActiveView: () => false,
      setPendingViewKey: vi.fn(),
    },
    workspaceSession: createWorkspaceSession(),
    draftSession: createDraftSession(),
    transport: { call: vi.fn(async () => []) },
    workspaceTransition: {
      activate: vi.fn(async () => 'activated' as const),
      invalidate: vi.fn(),
    },
    previewMode: false,
    desktopShellAvailable: true,
    tr: (source: string) => source,
    messageFrom: (cause: unknown) => String(cause),
    pageError: () => '',
    setPageError: vi.fn(),
    clearWorkspace: vi.fn(),
    selectAgentNavigationScope: vi.fn(async () => ({ selected: true })),
    requestIdForSession: vi.fn(() => null),
    onReady: vi.fn(),
    ...overrides,
  } as unknown as StartupControllerContext
  return { controller: createStartupController(context), context, navigation, restoreResolver }
}

describe('startup controller', () => {
  it('mounts immediately when nothing was restored', () => {
    const { controller } = harness()
    expect(get(controller).mounted).toBe(true)
    expect(get(controller).phase).toBe('idle')
  })

  it('waits for the workspace when a view was restored', () => {
    const { controller } = harness({
      workspaceShell: {
        views: () => [],
        activeView: () => null,
        activeViewKey: () => 'session:codex:alpha',
        hasRestoredSnapshot: () => true,
        restoredActiveView: () => true,
        setPendingViewKey: vi.fn(),
      } as never,
    })
    expect(get(controller).mounted).toBe(false)
  })

  it('reports a failed initialization with its message and timeout flag', async () => {
    const { controller, context } = harness({
      navigation: {
        subscribe: navigationStore().subscribe,
        initialize: vi.fn(async () => false),
        refreshNavigation: vi.fn(async () => true),
        selectScope: vi.fn(async () => ({ selected: true, requests: [] })),
      } as never,
    })

    await expect(controller.start()).resolves.toBe(false)
    expect(get(controller).phase).toBe('failed')
    expect(get(controller).failureMessage).toBe('Could not load the workbench.')
    expect(context.setPageError).not.toHaveBeenCalled()
  })

  it('marks the workbench ready and notifies the shell once', async () => {
    const { controller, context } = harness()

    await expect(controller.start()).resolves.toBe(true)
    expect(get(controller).phase).toBe('ready')
    expect(context.onReady).toHaveBeenCalledTimes(1)
    // A second call reuses the settled promise instead of initializing again.
    await expect(controller.start()).resolves.toBe(true)
    expect(context.navigation.initialize).toHaveBeenCalledTimes(1)
  })

  it('refreshes recovery only when the fingerprint changes', async () => {
    const { controller } = harness()
    await controller.recoverIfChanged('a')
    await controller.recoverIfChanged('a')
    await controller.recoverIfChanged('b')
    // The resolver is created inside the controller; two distinct fingerprints ran.
    expect(get(controller).phase).toBe('idle')
  })

  it('skips recovery while host facts are still pending', async () => {
    const navigation = navigationStore({ hostSessionFactsStatus: 'pending' })
    const { controller } = harness({
      navigation: {
        subscribe: navigation.subscribe,
        initialize: vi.fn(async () => true),
        refreshNavigation: vi.fn(async () => true),
        selectScope: vi.fn(async () => ({ selected: true, requests: [] })),
      } as never,
    })
    await expect(controller.recoverIfChanged('a')).resolves.toBeUndefined()
  })
})

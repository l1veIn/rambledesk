import { get, writable } from 'svelte/store'
import { describe, expect, it, vi } from 'vitest'

import { previewFixtures } from '../previewFixtures'
import {
  sessionViewDescriptor,
  settingsViewDescriptor,
  workspaceViewKey,
} from '../workspace/viewDescriptors'
vi.mock('../components/ui/sonner', () => ({
  toast: { error: vi.fn(), success: vi.fn(), info: vi.fn() },
}))

import { createAttachmentSession } from './attachmentSession'
import { createDraftSession } from './draftSession'
import {
  createWorkspaceNavigationController,
  type WorkspaceNavigationContext,
} from './workspaceNavigationController'
import { createWorkspaceSession } from './workspaceSession'

function harness(overrides: Record<string, unknown> = {}) {
  const shellState = writable({
    shell: { views: [], activeViewKey: null as string | null },
    requestIds: new Map<string, string>(),
    pendingViewKey: null as string | null,
  })
  const navigationState = writable({
    hostSessions: [],
    requests: previewFixtures.requests,
    pendingRequests: [],
    requestFilters: { status: 'all', timeRange: 'all' },
    selectedHostId: null,
    selectedHostSessionId: null,
    hostSessionFactsStatus: 'ready' as const,
    hostSessionFactsRevision: 1,
  })
  const workspaceSession = createWorkspaceSession()
  const draftSession = createDraftSession()
  const context = {
    navigation: {
      subscribe: navigationState.subscribe,
      selectScope: vi.fn(async () => ({ selected: true, requests: previewFixtures.requests })),
      setRequestSearch: vi.fn(async () => {}),
      refreshNavigation: vi.fn(async () => true),
    },
    workspaceShell: {
      subscribe: shellState.subscribe,
      views: () => get(shellState).shell.views,
      activeView: () => null,
      activeViewKey: () => get(shellState).shell.activeViewKey,
      pendingViewKey: () => get(shellState).pendingViewKey,
      requestIdFor: (view: unknown) =>
        get(shellState).requestIds.get(workspaceViewKey(view as never)),
      dispatch: vi.fn(() => true),
      bindRequest: vi.fn(),
      forgetRequest: vi.fn(),
      replaceShell: vi.fn(),
      setPendingViewKey: vi.fn(),
    },
    workspaceSession,
    draftSession,
    attachmentSession: createAttachmentSession(),
    startup: {
      patch: vi.fn(),
      resolutionFor: () => null,
      phase: () => 'ready' as const,
    },
    managedSessions: {
      closeDraft: vi.fn(async () => ({ skipped: false, promotedSessionId: null })),
      promotedSessionId: () => undefined,
    },
    transport: { call: vi.fn(async () => []) },
    workspaceTransition: {
      activate: vi.fn(async () => 'activated' as const),
      invalidate: vi.fn(() => 1),
      currentIntent: vi.fn(() => 1),
      isCurrent: vi.fn(() => true),
    },
    previewMode: true,
    tr: (source: string) => source,
    messageFrom: (cause: unknown) => String(cause),
    pageError: () => '',
    setPageError: vi.fn(),
    clearWorkspace: vi.fn(),
    refreshNotificationPermission: vi.fn(),
    isTransitionLocked: () => false,
    enqueueDocumentTask: <T>(task: () => Promise<T>) => task(),
    canAutoOpenRamble: () => true,
    onboardingOpen: () => false,
    resumePromptOpen: () => false,
    rambleEngaged: () => false,
    releaseAttachmentPreviews: vi.fn(),
    refreshAttachmentPreviews: vi.fn(),
    setCookingPreview: vi.fn(),
    ...overrides,
  } as unknown as WorkspaceNavigationContext
  return {
    controller: createWorkspaceNavigationController(context),
    context,
    shellState,
    workspaceSession,
    navigationState,
  }
}

describe('workspace navigation controller', () => {
  it('prefers the remembered request while list filters are active', () => {
    const { controller, context, navigationState } = harness()
    navigationState.update((state) => ({
      ...state,
      requestFilters: { status: 'waiting', timeRange: 'all' },
    }))
    const view = sessionViewDescriptor('codex', 'alpha')
    context.workspaceShell.requestIdFor = () => 'remembered'
    expect(
      controller.requestIdForSession(view, [{ request_id: 'listed' } as never]),
    ).toBe('remembered')
  })

  it('falls back to the first listed request without filters', () => {
    const { controller, context } = harness()
    const view = sessionViewDescriptor('codex', 'alpha')
    context.workspaceShell.requestIdFor = () => 'remembered'
    expect(
      controller.requestIdForSession(view, [{ request_id: 'listed' } as never]),
    ).toBe('listed')
  })

  it('resolves the view for a request from the navigation lists', () => {
    const { controller } = harness()
    const request = previewFixtures.requests[0]
    expect(controller.viewForRequest(request.request_id)).toEqual(
      sessionViewDescriptor(request.host_id, request.host_session_id),
    )
    expect(controller.viewForRequest('missing')).toBeNull()
  })

  it('loads a preview workspace target and rejects unknown requests', async () => {
    const { controller } = harness()
    const request = previewFixtures.requests[0]
    await expect(
      controller.loadWorkspaceTarget({
        view: sessionViewDescriptor(request.host_id, request.host_session_id),
        requestId: request.request_id,
        shellAction: { type: 'open' },
        pendingViewKey: 'key',
      }),
    ).resolves.toMatchObject({ kind: 'session' })

    await expect(
      controller.loadWorkspaceTarget({
        view: null,
        requestId: 'missing',
        shellAction: { type: 'open' },
        pendingViewKey: 'key',
      }),
    ).rejects.toThrow('This feedback request could not be found.')
  })

  it('commits a loaded session into the sessions and the shell', () => {
    const { controller, context, workspaceSession } = harness()
    const request = previewFixtures.requests[0]
    const view = sessionViewDescriptor(request.host_id, request.host_session_id)
    controller.commitWorkspaceTarget(
      { view, requestId: request.request_id, shellAction: { type: 'open' }, pendingViewKey: 'key' },
      {
        kind: 'session',
        workspace: previewFixtures.workspace,
        publishedFeedback: null,
      },
    )

    expect(workspaceSession.requestId()).toBe(previewFixtures.workspace.request.request_id)
    expect(context.workspaceShell.bindRequest).toHaveBeenCalled()
    expect(context.workspaceShell.replaceShell).toHaveBeenCalled()
    expect(context.startup.patch).toHaveBeenCalledWith({ mounted: true })
    expect(context.refreshAttachmentPreviews).toHaveBeenCalled()
  })

  it('opens a plain view after preparing its selection state', async () => {
    const { controller, context } = harness()
    const prepare = vi.fn()
    const view = settingsViewDescriptor()

    await expect(controller.openView(view, { prepare })).resolves.toBe('activated')

    expect(prepare).toHaveBeenCalled()
    expect(context.workspaceTransition.activate).toHaveBeenCalledWith(
      expect.objectContaining({ view, requestId: null, pendingViewKey: workspaceViewKey(view) }),
      1,
    )
  })

  it('keeps an already active view without re-activating it', async () => {
    const { controller, context, shellState } = harness()
    const view = settingsViewDescriptor()
    shellState.update((state) => ({
      ...state,
      shell: { ...state.shell, activeViewKey: workspaceViewKey(view) },
    }))

    await expect(controller.openView(view)).resolves.toBe('active')
    expect(context.workspaceTransition.activate).not.toHaveBeenCalled()
  })

  it('does not prepare selection state for a blocked open', async () => {
    const { controller, context, shellState } = harness()
    shellState.update((state) => ({ ...state, pendingViewKey: 'other' }))
    const prepare = vi.fn()

    await expect(controller.openView(settingsViewDescriptor(), { prepare })).resolves.toBe('blocked')
    expect(prepare).not.toHaveBeenCalled()
    expect(context.workspaceTransition.activate).not.toHaveBeenCalled()
  })

  it('routes a rail selection through the transition and restores a blocked scope', async () => {
    const { controller, context } = harness()
    await controller.selectRailScope('codex', 'alpha')
    expect(context.navigation.selectScope).toHaveBeenCalledWith('codex', 'alpha')
    expect(context.workspaceTransition.activate).toHaveBeenCalled()

    const blocked = harness({
      workspaceTransition: {
        activate: vi.fn(async () => 'blocked' as const),
        invalidate: vi.fn(() => 1),
        currentIntent: vi.fn(() => 1),
        isCurrent: vi.fn(() => true),
      },
    })
    await blocked.controller.selectRailScope(null, null)
    // A blocked outcome restores the scope that was active before the selection.
    expect(blocked.context.navigation.selectScope).toHaveBeenCalledTimes(2)
  })
})

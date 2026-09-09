import { tick } from 'svelte'
import { get } from 'svelte/store'

import { toast } from '../components/ui/sonner'
import { readApplicationSnapshot } from '../application/readApplicationSnapshot'
import type { ApplicationTransport } from '../application/applicationTransport'
import type { FeedbackRequestSummary, FeedbackWorkspaceView } from '../feedback'
import { normalizePublishedFeedback } from '../publishedFeedback'
import { agentSessionForView, arrivingRequestForAgentView } from '../workspace/agentViewRouting'
import { requestFilterCount } from '../domain/requestFilters'
import { leavesSettingsView } from '../workspace/workspaceViewLifecycle'
import {
  agentSessionViewDescriptor,
  inboxViewDescriptor,
  requestTaskViewDescriptor,
  sessionViewDescriptor,
  workspaceViewKey,
  type AgentSessionViewDescriptor,
  type SessionViewDescriptor,
  type WorkspaceViewDescriptor,
} from '../workspace/viewDescriptors'
import { activeWorkspaceView, workspaceShellReducer } from '../workspace/workspaceShell'
import type {
  WorkspaceTransitionOutcome,
  WorkspaceTransitionTarget,
} from '../workspace/workspaceTransition'
import type { AttachmentSession } from './attachmentSession'
import type { DraftSession } from './draftSession'
import type { ManagedSessionActions } from './managedSessionActions'
import type { createNavigationController } from './navigationController'
import { currentNavigationScope, restoreNavigationScope } from './navigationScope'
import type { StartupController } from './startupController'
import type { WorkspaceSession } from './workspaceSession'
import type { WorkspaceShellSession } from './workspaceShellSession'

type NavigationController = ReturnType<typeof createNavigationController>

type Transition = Readonly<{
  activate: (
    target: WorkspaceTransitionTarget,
    intent?: number,
    canLeave?: () => boolean,
  ) => Promise<WorkspaceTransitionOutcome>
  invalidate: () => number
  currentIntent: () => number
  isCurrent: (intent: number) => boolean
}>

export type LoadedWorkspaceTarget =
  | Readonly<{
      kind: 'session'
      workspace: FeedbackWorkspaceView
      publishedFeedback: { markdown: string; uncooked_markdown?: string } | null
    }>
  | Readonly<{ kind: 'request-task'; workspace: FeedbackWorkspaceView }>

/**
 * Workspace navigation: which view is active, which request it loads, and the
 * scope transitions that keep the rail consistent while a view swaps.
 *
 * Owns no state — every read goes through the sessions and controllers it is handed.
 */
export type WorkspaceNavigationContext = {
  navigation: NavigationController
  workspaceShell: WorkspaceShellSession
  workspaceSession: WorkspaceSession
  draftSession: DraftSession
  attachmentSession: AttachmentSession
  startup: StartupController
  managedSessions: ManagedSessionActions
  transport: ApplicationTransport
  workspaceTransition: Transition
  tr: (source: string, values?: Record<string, string | number>) => string
  messageFrom: (cause: unknown) => string
  pageError: () => string
  setPageError: (message: string) => void
  clearWorkspace: () => void
  refreshNotificationPermission: () => void
  releaseAttachmentPreviews: () => void
  refreshAttachmentPreviews: (workspace: FeedbackWorkspaceView) => void
  setCookingPreview: (preview: null) => void
  isTransitionLocked: () => boolean
  enqueueDocumentTask: <T>(task: () => Promise<T>) => Promise<T>
  canAutoOpenRamble: (sessionId: string) => boolean
  onboardingOpen: () => boolean
  resumePromptOpen: () => boolean
  rambleEngaged: () => boolean
}

export type WorkspaceNavigationController = ReturnType<typeof createWorkspaceNavigationController>

export function createWorkspaceNavigationController(context: WorkspaceNavigationContext) {
  function activeView() {
    return activeWorkspaceView(get(context.workspaceShell).shell)
  }

  function activeRequest() {
    return get(context.workspaceSession).workspace?.request ?? null
  }

  function requestIdForSession(
    view: SessionViewDescriptor,
    requests: readonly FeedbackRequestSummary[],
  ): string | null {
    const rememberedRequestId = context.workspaceShell.requestIdFor(view)
    // List filters may hide an open request; they must not reset its workspace tab.
    if (
      rememberedRequestId &&
      (requestFilterCount(get(context.navigation).requestFilters) > 0 ||
        requests.some((request) => request.request_id === rememberedRequestId))
    ) {
      return rememberedRequestId
    }
    return requests[0]?.request_id ?? null
  }

  function viewForRequest(requestId: string): SessionViewDescriptor | null {
    const navigation = get(context.navigation)
    const request = [...navigation.requests, ...navigation.pendingRequests].find(
      (candidate) => candidate.request_id === requestId,
    )
    const currentRequest = activeRequest()
    return request
      ? sessionViewDescriptor(request.host_id, request.host_session_id)
      : currentRequest?.request_id === requestId
        ? sessionViewDescriptor(currentRequest.host_id, currentRequest.host_session_id)
        : null
  }

  function openSessionView(view: SessionViewDescriptor, requestId?: string) {
    if (requestId) context.workspaceShell.bindRequest(workspaceViewKey(view), requestId)
    context.workspaceShell.dispatch({ type: 'open', view })
  }

  function reorderWorkspaceTabs(viewKeys: readonly string[]) {
    context.workspaceShell.dispatch({ type: 'reorder', viewKeys })
  }

  function openLoadedWorkspaceView(next: FeedbackWorkspaceView) {
    openSessionView(
      sessionViewDescriptor(next.request.host_id, next.request.host_session_id),
      next.request.request_id,
    )
  }

  async function selectAgentNavigationScope(view: AgentSessionViewDescriptor) {
    const session = agentSessionForView(view, get(context.navigation).hostSessions)
    return context.navigation.selectScope(session?.host_id ?? null, session?.host_session_id ?? null)
  }

  async function loadWorkspaceTarget(
    target: WorkspaceTransitionTarget,
  ): Promise<LoadedWorkspaceTarget | null> {
    if (!target.requestId) return null
    const requestId = target.requestId
    return context.enqueueDocumentTask(async () => {
      const next = await readApplicationSnapshot(context.transport, 'getFeedbackWorkspace', {
        request_id: requestId,
      })
      if (!next) throw new Error(context.tr('This feedback request could not be found.'))

      if (target.view?.kind === 'request-task') {
        if (target.view.requestId !== next.request.request_id) {
          throw new Error(context.tr('This feedback request could not be found.'))
        }
        return { kind: 'request-task', workspace: next }
      }

      const loadedView = sessionViewDescriptor(next.request.host_id, next.request.host_session_id)
      if (
        target.view?.kind === 'session' &&
        workspaceViewKey(target.view) !== workspaceViewKey(loadedView)
      ) {
        throw new Error(context.tr('The feedback request no longer belongs to the selected session.'))
      }
      if (target.view && target.view.kind !== 'session') {
        throw new Error(context.tr('This feedback request could not be found.'))
      }

      const nextPublishedFeedback =
        next.request.status === 'completed' && next.feedback
          ? normalizePublishedFeedback(
              await readApplicationSnapshot(context.transport, 'readPublishedFeedback', {
                request_id: next.request.request_id,
              }),
            )
          : null
      return { kind: 'session', workspace: next, publishedFeedback: nextPublishedFeedback }
    })
  }

  function commitWorkspaceTarget(
    target: WorkspaceTransitionTarget,
    loaded: LoadedWorkspaceTarget | null,
  ) {
    const previousActiveView = activeView()
    const requestedView = loaded?.kind === 'session'
      ? sessionViewDescriptor(
          loaded.workspace.request.host_id,
          loaded.workspace.request.host_session_id,
        )
      : loaded?.kind === 'request-task'
        ? requestTaskViewDescriptor(loaded.workspace.request.request_id)
        : target.view
    const promotedSessionId =
      requestedView?.kind === 'agent-draft'
        ? context.managedSessions.promotedSessionId(requestedView.draftId)
        : undefined
    const loadedView = promotedSessionId
      ? agentSessionViewDescriptor(promotedSessionId)
      : requestedView
    if (target.shellAction.type === 'open' && !loadedView) {
      throw new Error(context.tr('This feedback request could not be found.'))
    }

    const shell = get(context.workspaceShell).shell
    const nextShellState =
      target.shellAction.type === 'close'
        ? workspaceShellReducer(shell, target.shellAction)
        : workspaceShellReducer(shell, { type: 'open', view: loadedView! })

    if (loaded?.kind === 'session') {
      context.releaseAttachmentPreviews()
      context.workspaceSession.open(loaded.workspace, loaded.publishedFeedback)
      context.setCookingPreview(null)
      context.draftSession.adopt(loaded.workspace.draft)
      context.attachmentSession.setMessage('')
      context.workspaceShell.bindRequest(
        workspaceViewKey(loadedView!),
        loaded.workspace.request.request_id,
      )
      if (target.shellAction.type === 'close') {
        context.workspaceShell.forgetRequest(target.shellAction.viewKey)
      }
    } else if (loaded?.kind === 'request-task') {
      context.releaseAttachmentPreviews()
      context.workspaceSession.open(loaded.workspace)
      context.setCookingPreview(null)
      context.draftSession.adopt(loaded.workspace.draft)
      context.attachmentSession.setMessage('')
      if (target.shellAction.type === 'close') {
        context.workspaceShell.forgetRequest(target.shellAction.viewKey)
      }
    } else {
      context.clearWorkspace()
      if (target.shellAction.type === 'close') {
        context.workspaceShell.forgetRequest(target.shellAction.viewKey)
      }
    }

    context.workspaceShell.replaceShell(nextShellState)
    context.startup.patch({ mounted: true })
    context.workspaceSession.setLoading(false)
    if (leavesSettingsView(previousActiveView, activeWorkspaceView(nextShellState))) {
      context.refreshNotificationPermission()
    }
    if (loaded) void context.refreshAttachmentPreviews(loaded.workspace)
  }

  async function activateRequest(
    requestId: string,
    canLeaveCurrent: () => boolean = () => true,
  ): Promise<WorkspaceTransitionOutcome> {
    if (context.isTransitionLocked() || !canLeaveCurrent()) return 'blocked'
    const priorScope = currentNavigationScope(context.navigation)
    const intent = context.workspaceTransition.invalidate()
    const workspace = get(context.workspaceSession).workspace
    const currentView = activeView()
    if (workspace?.request.request_id === requestId && currentView?.kind === 'session') {
      openLoadedWorkspaceView(workspace)
      return 'activated'
    }
    context.setPageError('')
    const view = viewForRequest(requestId)
    // An exact request can load while its already-selected request list is refreshing.
    if (view && (view.hostId !== priorScope.hostId || view.hostSessionId !== priorScope.hostSessionId)) {
      const selection = await context.navigation.selectScope(view.hostId, view.hostSessionId)
      if (!context.workspaceTransition.isCurrent(intent)) return 'stale'
      if (!selection.selected) return 'failed'
    }
    if (context.isTransitionLocked() || !canLeaveCurrent()) {
      await restoreNavigationScope(context.navigation, priorScope, 'blocked')
      return 'blocked'
    }
    const outcome = await context.workspaceTransition.activate(
      {
        view,
        requestId,
        shellAction: { type: 'open' },
        pendingViewKey: view ? workspaceViewKey(view) : `request:${JSON.stringify(requestId)}`,
      },
      intent,
      canLeaveCurrent,
    )
    await restoreNavigationScope(context.navigation, priorScope, outcome)
    return outcome
  }

  async function openRequest(requestId: string): Promise<boolean> {
    return (await activateRequest(requestId)) === 'activated'
  }

  async function selectRailScope(hostId: string | null, hostSessionId: string | null) {
    if (context.isTransitionLocked()) return
    const priorScope = currentNavigationScope(context.navigation)
    const intent = context.workspaceTransition.invalidate()
    const selection = await context.navigation.selectScope(hostId, hostSessionId)
    if (!context.workspaceTransition.isCurrent(intent) || !selection.selected) return
    if (context.isTransitionLocked()) {
      await restoreNavigationScope(context.navigation, priorScope, 'blocked')
      return
    }
    if (!hostId || !hostSessionId) {
      const inbox = inboxViewDescriptor()
      const outcome = await context.workspaceTransition.activate(
        {
          view: inbox,
          requestId: null,
          shellAction: { type: 'open' },
          pendingViewKey: workspaceViewKey(inbox),
        },
        intent,
      )
      await restoreNavigationScope(context.navigation, priorScope, outcome)
      return
    }
    const view = sessionViewDescriptor(hostId, hostSessionId)
    const requestId = requestIdForSession(view, selection.requests)
    if (requestId) {
      const outcome = await activateRequest(requestId)
      await restoreNavigationScope(context.navigation, priorScope, outcome)
      return
    }
    const outcome = await context.workspaceTransition.activate(
      {
        view,
        requestId: null,
        shellAction: { type: 'open' },
        pendingViewKey: workspaceViewKey(view),
      },
      intent,
    )
    await restoreNavigationScope(context.navigation, priorScope, outcome)
  }

  /**
   * Opens a view that is not backed by a feedback request (settings, archive,
   * profile, request task). `prepare` runs after the transition guard and before
   * activation, so view-local selection state never updates for a blocked open.
   */
  async function openView(
    view: WorkspaceViewDescriptor,
    options: Readonly<{ requestId?: string | null; prepare?: () => void }> = {},
  ) {
    if (context.isTransitionLocked() || get(context.workspaceShell).pendingViewKey) {
      return 'blocked' as const
    }
    const viewKey = workspaceViewKey(view)
    options.prepare?.()
    if (get(context.workspaceShell).shell.activeViewKey === viewKey) return 'active' as const
    const intent = context.workspaceTransition.invalidate()
    return context.workspaceTransition.activate(
      {
        view,
        requestId: options.requestId ?? null,
        shellAction: { type: 'open' },
        pendingViewKey: viewKey,
      },
      intent,
    )
  }

  let lastAutoOpenedTaskRequestId = ''

  /** Opens the task brief for a request once; repeated triggers are ignored. */
  async function autoOpenTaskView(requestId: string) {
    if (lastAutoOpenedTaskRequestId === requestId) return
    lastAutoOpenedTaskRequestId = requestId
    await openView(requestTaskViewDescriptor(requestId), { requestId })
  }

  async function activateWorkspaceTab(viewKey: string) {
    if (context.isTransitionLocked() || get(context.workspaceShell).shell.activeViewKey === viewKey) {
      return
    }
    const priorScope = currentNavigationScope(context.navigation)
    const intent = context.workspaceTransition.invalidate()
    const view = get(context.workspaceShell).shell.views.find(
      (candidate) => workspaceViewKey(candidate) === viewKey,
    )
    if (!view) return
    if (view.kind !== 'session') {
      if (view.kind === 'inbox') {
        const selection = await context.navigation.selectScope(null, null)
        if (!selection.selected) return
      }
      if (view.kind === 'agent-session') {
        const selection = await selectAgentNavigationScope(view)
        if (!selection.selected) return
      }
      if (!context.workspaceTransition.isCurrent(intent)) return
      if (context.isTransitionLocked()) {
        await restoreNavigationScope(context.navigation, priorScope, 'blocked')
        return
      }
      const outcome = await context.workspaceTransition.activate(
        {
          view,
          requestId: view.kind === 'request-task' ? view.requestId : null,
          shellAction: { type: 'open' },
          pendingViewKey: viewKey,
        },
        intent,
      )
      if (view.kind === 'inbox' || view.kind === 'agent-session') {
        await restoreNavigationScope(context.navigation, priorScope, outcome)
      }
      return
    }
    const resolution = context.startup.resolutionFor(viewKey)
    if (resolution?.kind === 'missing-session') {
      const outcome = await context.workspaceTransition.activate(
        {
          view,
          requestId: null,
          shellAction: { type: 'open' },
          pendingViewKey: viewKey,
        },
        intent,
      )
      if (outcome === 'activated') await context.navigation.selectScope(null, null)
      return
    }

    const selection = await context.navigation.selectScope(view.hostId, view.hostSessionId)
    if (!context.workspaceTransition.isCurrent(intent) || !selection.selected) return
    if (context.isTransitionLocked()) {
      await context.navigation.selectScope(priorScope.hostId, priorScope.hostSessionId)
      return
    }
    const requestId = requestIdForSession(view, selection.requests)
    if (requestId) {
      const outcome = await activateRequest(requestId)
      await restoreNavigationScope(context.navigation, priorScope, outcome)
      return
    }
    const outcome = await context.workspaceTransition.activate(
      {
        view,
        requestId: null,
        shellAction: { type: 'open' },
        pendingViewKey: viewKey,
      },
      intent,
    )
    await restoreNavigationScope(context.navigation, priorScope, outcome)
  }

  async function closeWorkspaceTab(viewKey: string) {
    if (context.isTransitionLocked() || get(context.workspaceShell).pendingViewKey) return
    const closingView = get(context.workspaceShell).shell.views.find(
      (view) => workspaceViewKey(view) === viewKey,
    )
    if (closingView?.kind === 'agent-draft') {
      let closed: Readonly<{ skipped: boolean; promotedSessionId: string | null }>
      try {
        closed = await context.managedSessions.closeDraft(closingView.draftId)
      } catch (cause) {
        toast.error(context.messageFrom(cause))
        return
      }
      if (closed.skipped) return
      if (closed.promotedSessionId) {
        viewKey = workspaceViewKey(agentSessionViewDescriptor(closed.promotedSessionId))
      }
      // Another tab activation may have started while cleanup awaited the agent.
      // Its pending target owns the next mount; only remove the closed descriptor.
      if (get(context.workspaceShell).pendingViewKey) {
        context.workspaceShell.dispatch({ type: 'close', viewKey })
        return
      }
    }
    const closingActive = get(context.workspaceShell).shell.activeViewKey === viewKey
    if (!closingActive) {
      context.workspaceShell.dispatch({ type: 'close', viewKey })
      context.workspaceShell.forgetRequest(viewKey)
      return
    }
    const intent = context.workspaceTransition.invalidate()
    const priorScope = currentNavigationScope(context.navigation)

    const nextShellState = workspaceShellReducer(get(context.workspaceShell).shell, {
      type: 'close',
      viewKey,
    })
    const fallbackView = activeWorkspaceView(nextShellState)
    const fallbackResolution = fallbackView?.kind === 'session'
      ? context.startup.resolutionFor(workspaceViewKey(fallbackView))
      : null
    let fallbackRequestId: string | null = null
    if (fallbackView?.kind === 'session' && fallbackResolution?.kind !== 'missing-session') {
      const selection = await context.navigation.selectScope(
        fallbackView.hostId,
        fallbackView.hostSessionId,
      )
      if (!context.workspaceTransition.isCurrent(intent) || !selection.selected) return
      if (context.isTransitionLocked()) {
        await context.navigation.selectScope(priorScope.hostId, priorScope.hostSessionId)
        return
      }
      fallbackRequestId = requestIdForSession(fallbackView, selection.requests)
    } else if (fallbackView?.kind === 'inbox') {
      const selection = await context.navigation.selectScope(null, null)
      if (!selection.selected) return
    } else if (fallbackView?.kind === 'request-task') {
      fallbackRequestId = fallbackView.requestId
    } else if (fallbackView?.kind === 'agent-session') {
      const selection = await selectAgentNavigationScope(fallbackView)
      if (!selection.selected) return
    }

    if (!context.workspaceTransition.isCurrent(intent)) return
    if (context.isTransitionLocked()) {
      await restoreNavigationScope(context.navigation, priorScope, 'blocked')
      return
    }
    const outcome = await context.workspaceTransition.activate(
      {
        view: fallbackView,
        requestId: fallbackRequestId,
        shellAction: { type: 'close', viewKey },
        pendingViewKey: viewKey,
      },
      intent,
    )
    if (
      outcome === 'activated' &&
      (!fallbackView ||
        (fallbackView.kind === 'session' && fallbackResolution?.kind === 'missing-session'))
    ) {
      await context.navigation.selectScope(null, null)
    }
    await restoreNavigationScope(context.navigation, priorScope, outcome)
  }

  async function searchWorkspaceRequests(search: string) {
    if (context.isTransitionLocked()) return
    const intent = context.workspaceTransition.invalidate()
    await context.navigation.setRequestSearch(search)
    if (!context.workspaceTransition.isCurrent(intent) || context.isTransitionLocked()) return
    await selectRailScope(null, null)
  }

  async function autoOpenArrivingRequest(arrivals: readonly FeedbackRequestSummary[]) {
    const origin = activeView()
    if (origin?.kind !== 'agent-session' || get(context.workspaceShell).pendingViewKey) return
    const intent = context.workspaceTransition.currentIntent()
    // Draft promotion can replace the active view in the same update as the first request.
    await tick()
    if (!context.workspaceTransition.isCurrent(intent) || get(context.workspaceShell).pendingViewKey) {
      return
    }
    const canLeave = () => {
      const current = activeView()
      return (
        current?.kind === 'agent-session' &&
        current.sessionId === origin.sessionId &&
        context.startup.phase() === 'ready' &&
        !context.onboardingOpen() &&
        !context.resumePromptOpen() &&
        !context.isTransitionLocked() &&
        !context.rambleEngaged() &&
        context.canAutoOpenRamble(origin.sessionId) === true
      )
    }
    const request = arrivingRequestForAgentView(origin, arrivals, canLeave())
    if (request) await activateRequest(request.request_id, canLeave)
  }

  return {
    loadWorkspaceTarget,
    commitWorkspaceTarget,
    activateRequest,
    openRequest,
    selectRailScope,
    activateWorkspaceTab,
    openView,
    autoOpenTaskView,
    closeWorkspaceTab,
    searchWorkspaceRequests,
    autoOpenArrivingRequest,
    openSessionView,
    reorderWorkspaceTabs,
    openLoadedWorkspaceView,
    viewForRequest,
    requestIdForSession,
    selectAgentNavigationScope,
  }
}

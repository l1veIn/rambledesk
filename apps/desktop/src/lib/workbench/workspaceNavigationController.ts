import { tick } from 'svelte'
import { get } from 'svelte/store'

import { toast } from '../components/ui/sonner'
import { readApplicationSnapshot } from '../application/readApplicationSnapshot'
import {
  applicationResourcesAffectNavigation,
  applicationResourcesAffectWorkspace,
  applicationResourcesRequireFullNavigationSnapshot,
  createApplicationSnapshotRefetch,
  type ApplicationSnapshotRefetchIntent,
} from '../application/applicationSnapshotRefetch'
import type { ApplicationTransport } from '../application/applicationTransport'
import type { ApplicationResourceKey } from '../generated/feedback'
import type { FeedbackRequestSummary, FeedbackWorkspaceView } from '../feedback'
import { restoreFeedbackDraftDocument, snapshotFeedbackDraftDocument } from '../feedbackDraftDocument'
import { normalizePublishedFeedback } from '../publishedFeedback'
import { agentSessionForView, arrivingRequestForAgentView, latestPendingRequestForSession } from '../workspace/agentViewRouting'
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
import { workspaceShellReducer } from '../workspace/workspaceShell'
import {
  createWorkspaceTransition,
  type WorkspaceShellIntent,
  type WorkspaceTransitionOutcome,
  type WorkspaceTransitionTarget,
} from '../workspace/workspaceTransition'
import type { AttachmentController } from './attachmentController'
import type { AttachmentSession } from './attachmentSession'
import type { CookingSession } from './cookingSession'
import type { DraftController } from './draftController'
import type { DraftSession } from './draftSession'
import type { ManagedSessionActions } from './managedSessionActions'
import type { createNavigationController } from './navigationController'
import type { NavigationScope, PreparedNavigationScope } from './navigationScope'
import type { StartupController } from './startupController'
import type { WorkspaceSession } from './workspaceSession'
import type { WorkspaceShellSession } from './workspaceShellSession'

type LoadedWorkspaceTarget = Readonly<{
  kind: 'session' | 'request-task'
  workspace: FeedbackWorkspaceView
  publishedFeedback: { markdown: string; uncooked_markdown?: string } | null
  scope?: PreparedNavigationScope
}>

type NavigationTarget = WorkspaceTransitionTarget & Readonly<{
  scope?: PreparedNavigationScope
  isCurrent: () => boolean
  prepare?: () => void
}>

type ActivationOptions = Readonly<{
  requestId?: string | null
  expectedIntent?: number
  canLeave?: () => boolean
  missingSession?: boolean
}>

type NavigationOptions = ActivationOptions & Readonly<{
  scope?: NavigationScope
  shellAction?: WorkspaceShellIntent
  prepare?: () => void
  ignoreTransitionLock?: boolean
}>

/**
 * Owns the complete workspace change: prepare the rail scope, save, unmount,
 * load, then accept the scope and workspace together. Failed or stale candidates
 * never become visible. The existing sessions remain the owners of their facts.
 */
export type WorkspaceNavigationContext = {
  navigation: ReturnType<typeof createNavigationController>
  workspaceShell: WorkspaceShellSession
  workspaceSession: WorkspaceSession
  draftSession: DraftSession
  draftController: Pick<DraftController, 'saveDraftNow'>
  attachmentSession: AttachmentSession
  attachmentController: Pick<AttachmentController, 'releasePreviews' | 'refreshPreviews'>
  cookingSession: Pick<CookingSession, 'setPreview'>
  // Startup and managed-session actions call back into navigation. These two
  // owner references are resolved only when an operation runs, after composition.
  startup: () => Pick<StartupController, 'patch' | 'phase' | 'resolutionFor' | 'refreshSessionViewRecovery'>
  managedSessions: () => Pick<ManagedSessionActions, 'closeDraft' | 'promotedSessionId'>
  transport: ApplicationTransport
  tr: (source: string, values?: Record<string, string | number>) => string
  messageFrom: (cause: unknown) => string
  setPageError: (message: string) => void
  releaseEditor: () => void
  refreshNotificationPermission: () => void
  isTransitionLocked: () => boolean
  enqueueDocumentTask: <T>(task: () => Promise<T>) => Promise<T>
  onboardingOpen: () => boolean
  resumePromptOpen: () => boolean
}

export type WorkspaceNavigationController = ReturnType<typeof createWorkspaceNavigationController>

export function createWorkspaceNavigationController(context: WorkspaceNavigationContext) {
  let disposed = false
  let lastAutoOpenedTaskRequestId: string | null = null
  const activeView = () => context.workspaceShell.activeView()

  function clearWorkspace() {
    context.workspaceSession.close()
    context.draftSession.reset()
    context.attachmentController.releasePreviews()
    context.cookingSession.setPreview(null)
  }

  function restoreCurrent() {
    context.startup().patch({ mounted: true })
    context.workspaceSession.setLoading(false)
  }

  const transition = createWorkspaceTransition<LoadedWorkspaceTarget, NavigationTarget>({
    saveCurrent: context.draftController.saveDraftNow,
    unmountCurrent: () => {
      context.startup().patch({ mounted: false })
      context.releaseEditor()
      context.workspaceSession.setLoading(true)
    },
    loadTarget: loadWorkspaceTarget,
    commitTarget: commitWorkspaceTarget,
    restoreCurrent,
    setPendingTarget: (target) => context.workspaceShell.setPendingViewKey(target?.pendingViewKey ?? null),
    reportFailure: (cause) => {
      if (context.startup().phase() === 'loading') context.startup().patch({ workspaceFailure: cause })
      context.setPageError(context.messageFrom(cause))
    },
  })

  function requestIdForSession(view: SessionViewDescriptor, requests: readonly FeedbackRequestSummary[]): string | null {
    const remembered = context.workspaceShell.requestIdFor(view)
    // Filters can hide an open request without invalidating the tab's binding.
    if (remembered && (requestFilterCount(get(context.navigation).requestFilters) > 0 || requests.some(request => request.request_id === remembered))) {
      return remembered
    }
    return requests[0]?.request_id ?? null
  }

  function viewForRequest(requestId: string): SessionViewDescriptor | null {
    const navigation = get(context.navigation)
    const request = [...navigation.requests, ...navigation.pendingRequests, context.workspaceSession.request()]
      .find(candidate => candidate?.request_id === requestId)
    return request ? sessionViewDescriptor(request.host_id, request.host_session_id) : null
  }

  function scopeForAgent(view: AgentSessionViewDescriptor): NavigationScope {
    const session = agentSessionForView(view, get(context.navigation).hostSessions)
    return { hostId: session?.host_id ?? null, hostSessionId: session?.host_session_id ?? null }
  }

  function scopeForView(view: WorkspaceViewDescriptor | null, missingSession = false): NavigationScope | undefined {
    if (!view || view.kind === 'inbox' || missingSession) return { hostId: null, hostSessionId: null }
    if (view.kind === 'session') return { hostId: view.hostId, hostSessionId: view.hostSessionId }
    if (view.kind === 'agent-session') return scopeForAgent(view)
    return undefined
  }

  async function loadWorkspaceTarget(target: NavigationTarget): Promise<LoadedWorkspaceTarget | null> {
    if (!target.requestId) return null
    return context.enqueueDocumentTask(async () => {
      const workspace = await readApplicationSnapshot(context.transport, 'getFeedbackWorkspace', { request_id: target.requestId! })
      if (!target.isCurrent()) return null
      if (!workspace || workspace.request.request_id !== target.requestId) throw new Error(context.tr('The request no longer exists.'))
      const view = sessionViewDescriptor(workspace.request.host_id, workspace.request.host_session_id)
      if (target.view?.kind === 'request-task') {
        if (workspace.request.request_id !== target.view.requestId) throw new Error(context.tr('The request no longer exists.'))
      } else if (target.view && (target.view.kind !== 'session' || workspaceViewKey(target.view) !== workspaceViewKey(view))) {
        throw new Error(context.tr('The request no longer belongs to this session.'))
      }
      // Requests opened from an external notification may be absent from all
      // navigation lists. Their loaded identity determines the scope to accept.
      const scope = !target.view && !target.scope
        ? await context.navigation.prepareScope(view.hostId, view.hostSessionId, target.isCurrent)
        : undefined
      if (scope === null && target.isCurrent()) throw new Error(context.tr('The session could not be opened.'))
      const publishedFeedback = workspace.request.status === 'completed' && workspace.request.resolution === 'feedback_submitted'
        ? normalizePublishedFeedback(await readApplicationSnapshot(context.transport, 'readPublishedFeedback', { request_id: workspace.request.request_id }))
        : null
      return { kind: target.view?.kind === 'request-task' ? 'request-task' : 'session', workspace, publishedFeedback, ...(scope ? { scope } : {}) }
    })
  }

  function commitWorkspaceTarget(target: NavigationTarget, loaded: LoadedWorkspaceTarget | null) {
    const previousActive = activeView()
    let view = loaded
      ? loaded.kind === 'request-task'
        ? requestTaskViewDescriptor(loaded.workspace.request.request_id)
        : sessionViewDescriptor(loaded.workspace.request.host_id, loaded.workspace.request.host_session_id)
      : target.view
    if (view?.kind === 'agent-draft') {
      const promoted = context.managedSessions().promotedSessionId(view.draftId)
      if (promoted) view = agentSessionViewDescriptor(promoted)
    }
    if (target.shellAction.type === 'open' && !view) throw new Error(context.tr('The request no longer exists.'))
    const scope = loaded?.scope ?? target.scope
    if (scope && !context.navigation.canCommitScope(scope)) throw new Error(context.tr('The navigation scope changed.'))
    target.prepare?.()
    if (scope) context.navigation.commitScope(scope)
    const shell = workspaceShellReducer(get(context.workspaceShell).shell, target.shellAction.type === 'close'
      ? { type: 'close', viewKey: target.shellAction.viewKey }
      : { type: 'open', view: view! })
    if (loaded) {
      context.attachmentController.releasePreviews()
      context.workspaceSession.open(loaded.workspace, loaded.publishedFeedback)
      context.draftSession.adopt(loaded.workspace.draft)
      context.attachmentSession.setMessage('')
      context.cookingSession.setPreview(null)
      if (view?.kind === 'session') context.workspaceShell.bindRequest(workspaceViewKey(view), loaded.workspace.request.request_id)
    } else {
      clearWorkspace()
    }
    if (target.shellAction.type === 'close') context.workspaceShell.forgetRequest(target.shellAction.viewKey)
    context.workspaceShell.replaceShell(shell)
    restoreCurrent()
    if (leavesSettingsView(previousActive, view)) context.refreshNotificationPermission()
    if (loaded) void context.attachmentController.refreshPreviews(loaded.workspace)
  }

  async function navigate(view: WorkspaceViewDescriptor | null, options: NavigationOptions = {}): Promise<WorkspaceTransitionOutcome> {
    if (disposed) return 'stale'
    if (options.expectedIntent !== undefined && !transition.isCurrent(options.expectedIntent)) return 'stale'
    const canLeave = () => !disposed && (options.ignoreTransitionLock || !context.isTransitionLocked()) && (options.canLeave?.() ?? true)
    if (!canLeave()) return 'blocked'
    const intent = options.expectedIntent ?? transition.invalidate()
    const isCurrent = () => !disposed && transition.isCurrent(intent)
    const pendingViewKey = view ? workspaceViewKey(view) : options.shellAction?.type === 'close'
      ? options.shellAction.viewKey : 'request:' + JSON.stringify(options.requestId)
    context.workspaceShell.setPendingViewKey(pendingViewKey)
    context.setPageError('')
    try {
      const missingSession = options.missingSession || (view?.kind === 'session' && context.startup().resolutionFor(workspaceViewKey(view))?.kind === 'missing-session')
      // An unknown request discovers its scope when its workspace is loaded.
      const scope = options.scope ?? (view || !options.requestId ? scopeForView(view, missingSession) : undefined)
      const prepared = scope ? await context.navigation.prepareScope(scope.hostId, scope.hostSessionId, isCurrent) : undefined
      if (!isCurrent()) return 'stale'
      if (scope && !prepared) return 'failed'
      if (!canLeave() || (prepared && !context.navigation.canCommitScope(prepared))) return 'blocked'
      if (view && view.kind !== 'session' && view.kind !== 'request-task' &&
        context.workspaceShell.activeViewKey() === workspaceViewKey(view) &&
        context.workspaceSession.requestId() === null && options.shellAction?.type !== 'close') {
        // Promotion can change the rail scope while preserving the already
        // mounted Agent composer and its local, unsent input.
        options.prepare?.()
        if (prepared) context.navigation.commitScope(prepared)
        restoreCurrent()
        return 'activated'
      }
      const requestId = options.requestId !== undefined ? options.requestId
        : missingSession ? null
        : view?.kind === 'request-task' ? view.requestId
        : view?.kind === 'session' ? requestIdForSession(view, prepared?.requests ?? [])
        : null
      const target: NavigationTarget = {
        view, requestId, pendingViewKey,
        shellAction: options.shellAction ?? { type: 'open' },
        ...(prepared ? { scope: prepared } : {}),
        isCurrent,
        prepare: options.prepare,
      }
      return await transition.activate(target, intent, () => canLeave() && (!prepared || context.navigation.canCommitScope(prepared)))
    } finally {
      if (isCurrent()) context.workspaceShell.setPendingViewKey(null)
    }
  }

  function activateView(view: WorkspaceViewDescriptor, options: ActivationOptions = {}) {
    return navigate(view, options)
  }

  async function activateRequest(requestId: string, canLeave: () => boolean = () => true): Promise<WorkspaceTransitionOutcome> {
    if (disposed || context.isTransitionLocked() || !canLeave()) return 'blocked'
    if (context.workspaceSession.requestId() === requestId && activeView()?.kind === 'session') {
      transition.invalidate()
      return 'activated'
    }
    return navigate(viewForRequest(requestId), { requestId, canLeave })
  }

  async function openRequest(requestId: string): Promise<boolean> {
    return await activateRequest(requestId) === 'activated'
  }

  async function openView(view: WorkspaceViewDescriptor, options: { requestId?: string | null; prepare?: () => void } = {}) {
    if (disposed || context.isTransitionLocked()) return 'blocked' as const
    if (context.workspaceShell.activeViewKey() === workspaceViewKey(view)) {
      transition.invalidate()
      options.prepare?.()
      return 'active' as const
    }
    return navigate(view, options)
  }

  async function selectRailScope(hostId: string | null, hostSessionId: string | null) {
    return navigate(hostId && hostSessionId ? sessionViewDescriptor(hostId, hostSessionId) : inboxViewDescriptor(), {
      scope: { hostId, hostSessionId },
    })
  }

  async function activateWorkspaceTab(viewKey: string) {
    const view = context.workspaceShell.views().find(candidate => workspaceViewKey(candidate) === viewKey)
    if (!view) return 'stale' as const
    if (context.workspaceShell.activeViewKey() === viewKey) {
      if (!context.isTransitionLocked()) transition.invalidate()
      return 'activated' as const
    }
    return navigate(view)
  }

  async function closeWorkspaceTab(viewKey: string): Promise<WorkspaceTransitionOutcome> {
    if (disposed) return 'stale'
    if (context.isTransitionLocked()) return 'blocked'
    let view = context.workspaceShell.views().find(candidate => workspaceViewKey(candidate) === viewKey)
    if (!view) return 'stale'
    const intent = context.workspaceShell.activeViewKey() === viewKey ? transition.invalidate() : transition.currentIntent()
    if (view.kind === 'agent-draft') {
      try {
        const closed = await context.managedSessions().closeDraft(view.draftId)
        if (closed.skipped) return 'blocked'
        if (closed.promotedSessionId) viewKey = workspaceViewKey(agentSessionViewDescriptor(closed.promotedSessionId))
      } catch (cause) {
        if (!disposed) toast.error(context.messageFrom(cause))
        return 'failed'
      }
    }
    if (disposed) return 'stale'
    if (context.workspaceShell.activeViewKey() !== viewKey) {
      context.workspaceShell.dispatch({ type: 'close', viewKey })
      context.workspaceShell.forgetRequest(viewKey)
      return 'activated'
    }
    if (!transition.isCurrent(intent)) return 'stale'
    const nextShell = workspaceShellReducer(get(context.workspaceShell).shell, { type: 'close', viewKey })
    const fallback = nextShell.views.find(candidate => workspaceViewKey(candidate) === nextShell.activeViewKey) ?? null
    return navigate(fallback, { shellAction: { type: 'close', viewKey }, expectedIntent: intent })
  }

  async function autoOpenTaskView(requestId: string) {
    if (requestId === lastAutoOpenedTaskRequestId) return
    const outcome = await openView(requestTaskViewDescriptor(requestId), { requestId })
    if (outcome === 'active' || outcome === 'activated') lastAutoOpenedTaskRequestId = requestId
  }

  async function searchWorkspaceRequests(search: string) {
    if (disposed || context.isTransitionLocked()) return
    const intent = transition.invalidate()
    await context.navigation.setRequestSearch(search)
    await navigate(inboxViewDescriptor(), { expectedIntent: intent })
  }

  async function autoOpenArrivingRequest(arrivals: readonly FeedbackRequestSummary[]) {
    const origin = activeView()
    if (disposed || context.workspaceShell.pendingViewKey()) return
    if (origin?.kind !== 'agent-session' && origin?.kind !== 'session' && origin?.kind !== 'request-task') return
    // Inbox refresh also updates host-session facts; that must not cancel this jump.
    await tick()
    if (disposed || context.workspaceShell.pendingViewKey()) return
    const stillWatching = () => {
      const current = activeView()
      if (context.startup().phase() !== 'ready' || context.onboardingOpen() || context.resumePromptOpen()) return false
      if (origin.kind === 'agent-session') {
        return current?.kind === 'agent-session' && current.sessionId === origin.sessionId
      }
      if (origin.kind === 'session') {
        return current?.kind === 'session' && current.hostId === origin.hostId && current.hostSessionId === origin.hostSessionId
      }
      return current?.kind === 'request-task' && current.requestId === origin.requestId
    }
    const fromAgent = origin.kind === 'agent-session'
    const request = arrivingRequestForAgentView(origin, arrivals, stillWatching(), context.workspaceSession.request())
    if (!request) return
    context.navigation.revealRequest(request)
    if (context.workspaceSession.requestId() === request.request_id && activeView()?.kind === 'session') return
    await navigate(sessionViewDescriptor(request.host_id, request.host_session_id), {
      requestId: request.request_id,
      canLeave: stillWatching,
      ignoreTransitionLock: fromAgent,
    })
    if (disposed || activeView()?.kind !== 'session') return
    const latest = latestPendingRequestForSession(request, get(context.navigation).requests) ?? request
    context.navigation.revealRequest(latest)
    if (latest.request_id === context.workspaceSession.requestId()) return
    await navigate(sessionViewDescriptor(latest.host_id, latest.host_session_id), {
      requestId: latest.request_id,
      ignoreTransitionLock: fromAgent,
    })
  }

  async function refetchApplicationSnapshots(refetch: ApplicationSnapshotRefetchIntent): Promise<void> {
    const intent = transition.currentIntent()
    const view = activeView()
    const workspace = get(context.workspaceSession).workspace
    const isCurrent = () => !disposed && refetch.isCurrent() && transition.isCurrent(intent)
    if (applicationResourcesAffectNavigation(refetch.resources)) {
      if (applicationResourcesRequireFullNavigationSnapshot(refetch.resources)) await context.navigation.initialize(false)
      else await context.navigation.refreshNavigation(true)
      if (!isCurrent()) return
      await context.startup().refreshSessionViewRecovery()
    }
    // A user intent has priority for the entire wait and load, not just at the
    // instant a refresh finally sees an unlocked workspace.
    while (isCurrent() && (context.isTransitionLocked() || context.workspaceShell.pendingViewKey())) {
      await new Promise(resolve => setTimeout(resolve, 50))
    }
    if (!isCurrent() || !view || !workspace || (view.kind !== 'session' && view.kind !== 'request-task') ||
      context.workspaceShell.activeViewKey() !== workspaceViewKey(view) || context.workspaceSession.requestId() !== workspace.request.request_id ||
      !applicationResourcesAffectWorkspace(refetch.resources, {
        requestId: workspace.request.request_id, hostId: workspace.request.host_id, hostSessionId: workspace.request.host_session_id,
      })) return

    // A save broadcasts the same workspace back to its author. Refresh those
    // facts in place: navigating would destroy selection and Undo after every save.
    if (!await context.draftController.saveDraftNow() || !isCurrent()) return
    const refreshed = await context.enqueueDocumentTask(() => readApplicationSnapshot(
      context.transport, 'getFeedbackWorkspace', { request_id: workspace.request.request_id },
    ))
    if (!isCurrent() || context.workspaceShell.activeViewKey() !== workspaceViewKey(view) ||
      context.workspaceSession.requestId() !== workspace.request.request_id) return
    const draft = get(context.draftSession)
    const currentRequest = context.workspaceSession.request()
    if (refreshed && refreshed.request.request_id === workspace.request.request_id &&
      refreshed.request.status === currentRequest?.status && refreshed.request.resolution === currentRequest.resolution &&
      !context.workspaceSession.isTerminal() && !context.isTransitionLocked() && !context.workspaceShell.pendingViewKey() &&
      refreshed.draft.saved_revision >= draft.savedRevision) {
      const remote = snapshotFeedbackDraftDocument(restoreFeedbackDraftDocument(refreshed.draft.document_json, refreshed.draft.body_markdown))
      // Input may arrive during the read. A known saved baseline can refresh
      // metadata without replacing those newer local edits either.
      if (remote.documentJson === draft.documentJson ||
        (refreshed.draft.saved_revision === draft.savedRevision && remote.documentJson === draft.savedDocumentJson)) {
        context.workspaceSession.replace(refreshed)
        context.draftSession.reconcile(refreshed.draft)
        void context.attachmentController.refreshPreviews(refreshed)
        return
      }
    }
    await navigate(view, { requestId: workspace.request.request_id, expectedIntent: intent, canLeave: isCurrent })
  }

  const refetch = createApplicationSnapshotRefetch({
    refetch: refetchApplicationSnapshots,
    reportError: cause => { if (!disposed) context.setPageError(context.messageFrom(cause)) },
  })

  function dispose() {
    disposed = true
    refetch.dispose()
    transition.invalidate()
  }

  return {
    openRequest, activateRequest, activateView, openView, selectRailScope, activateWorkspaceTab, closeWorkspaceTab,
    autoOpenTaskView, searchWorkspaceRequests, autoOpenArrivingRequest,
    reorderWorkspaceTabs: (viewKeys: readonly string[]) => context.workspaceShell.dispatch({ type: 'reorder', viewKeys }),
    requestRefetch: (resources: readonly ApplicationResourceKey[]) => refetch.request(resources),
    refetchAfterTransportReady: () => refetch.request([{ kind: 'all' }]),
    // Recovery and managed-session promotion can cancel or observe navigation;
    // they cannot bypass workspace/scoping rules with a raw transition target.
    invalidate: transition.invalidate,
    currentIntent: transition.currentIntent,
    isCurrent: transition.isCurrent,
    clearWorkspace,
    dispose,
  }
}

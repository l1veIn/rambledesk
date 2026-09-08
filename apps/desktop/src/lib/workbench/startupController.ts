import { get, writable } from 'svelte/store'

import { ApplicationReadTimeoutError, withApplicationReadTimeout } from '../application/applicationReadTimeout'
import { readApplicationSnapshot } from '../application/readApplicationSnapshot'
import type { ApplicationTransport } from '../application/applicationTransport'
import type { FeedbackRequestSummary } from '../feedback'
import { startClientDiagnostic, diagnosticErrorCategory } from '../diagnostics/clientDiagnostics'
import { previewFixtures } from '../previewFixtures'
import { agentSessionForView, cancelledFeedbackRestoreTarget } from '../workspace/agentViewRouting'
import {
  createSessionViewRecoveryResolver,
  preserveLoadedSessionDuringUnconfirmedRecovery,
  sessionViewResolution,
  type SessionViewCatalog,
  type SessionViewResolution,
} from '../workspace/sessionViewRecovery'
import type {
  WorkspaceTransitionOutcome,
  WorkspaceTransitionTarget,
} from '../workspace/workspaceTransition'
import { activeWorkspaceView } from '../workspace/workspaceShell'
import { sessionViewDescriptor, workspaceViewKey } from '../workspace/viewDescriptors'
import type { AgentSessionViewDescriptor } from '../workspace/viewDescriptors'
import type { DraftSession } from './draftSession'
import type { createNavigationController } from './navigationController'
import type { WorkspaceSession } from './workspaceSession'
import type { WorkspaceShellSession } from './workspaceShellSession'

/**
 * Workbench startup and session-view recovery.
 *
 * Owns whether the workbench is mounted, the startup failure surface and the session
 * view resolutions the shell renders. Orchestration only: state lives in the sessions
 * it is handed.
 */
export type StartupState = Readonly<{
  phase: 'idle' | 'loading' | 'failed' | 'ready'
  mounted: boolean
  failureMessage: string
  failureTimedOut: boolean
  settingsOpen: boolean
  workspaceFailure: unknown
}>

type NavigationController = ReturnType<typeof createNavigationController>

type Transition = Readonly<{
  activate: (
    target: WorkspaceTransitionTarget,
    intent?: number,
  ) => Promise<WorkspaceTransitionOutcome>
  invalidate: () => unknown
}>

export type StartupControllerContext = {
  navigation: NavigationController
  workspaceShell: WorkspaceShellSession
  workspaceSession: WorkspaceSession
  draftSession: DraftSession
  transport: ApplicationTransport
  workspaceTransition: Transition
  previewMode: boolean
  desktopShellAvailable: boolean
  tr: (source: string, values?: Record<string, string | number>) => string
  messageFrom: (cause: unknown) => string
  pageError: () => string
  setPageError: (message: string) => void
  clearWorkspace: () => void
  selectAgentNavigationScope: (
    view: AgentSessionViewDescriptor,
  ) => Promise<Readonly<{ selected: boolean }>>
  requestIdForSession: (
    view: Extract<ReturnType<WorkspaceShellSession['activeView']>, { kind: 'session' }>,
    requests: readonly FeedbackRequestSummary[],
  ) => string | null
  /** Called once the workbench is ready so the shell can start polling. */
  onReady: () => void
}

const initial: StartupState = {
  phase: 'idle',
  mounted: true,
  failureMessage: '',
  failureTimedOut: false,
  settingsOpen: false,
  workspaceFailure: null,
}

export type StartupController = ReturnType<typeof createStartupController>

export function createStartupController(context: StartupControllerContext) {
  // A restored view mounts only after its workspace loads.
  const store = writable<StartupState>({
    ...initial,
    mounted: !context.workspaceShell.restoredActiveView(),
  })
  let ready: Promise<boolean> | null = null
  let sessionViewResolutions: readonly SessionViewResolution[] = []
  let lastRecoveryFingerprint = ''
  let activeRecoveryTransition: object | null = null

  function patch(next: Partial<StartupState>) {
    store.update((current) => ({ ...current, ...next }))
  }

  function activeSessionCatalog(): SessionViewCatalog {
    const navigation = get(context.navigation)
    if (navigation.hostSessionFactsStatus === 'pending') return { status: 'pending' }
    if (navigation.hostSessionFactsStatus === 'failed') return { status: 'failed' }
    return {
      status: 'ready',
      views: navigation.hostSessions.map((session) =>
        sessionViewDescriptor(session.host_id, session.host_session_id),
      ),
    }
  }

  const recoveryResolver = createSessionViewRecoveryResolver({
    loadArchived: async () => {
      const sessions = context.previewMode
        ? previewFixtures.archivedHostSessions
        : await context.transport.call('listArchivedHostSessions', { search: null })
      return sessions.map((session) =>
        sessionViewDescriptor(session.host_id, session.host_session_id),
      )
    },
    onInvalidate: () => {
      if (!activeRecoveryTransition) return
      activeRecoveryTransition = null
      context.workspaceTransition.invalidate()
    },
    onUpdate: applySessionViewResolutions,
  })

  async function refreshSessionViewRecovery() {
    return recoveryResolver.refresh(
      context.workspaceShell.views().filter(
        (view): view is Extract<typeof view, { kind: 'session' }> => view.kind === 'session',
      ),
      activeSessionCatalog(),
    )
  }

  function resolutionFor(viewKey: string | null): SessionViewResolution | null {
    return sessionViewResolution(sessionViewResolutions, viewKey)
  }

  function phase() {
    return get(store).phase
  }

  function resolutions(): readonly SessionViewResolution[] {
    return sessionViewResolutions
  }

  async function applySessionViewResolutions(
    resolutionsToApply: readonly SessionViewResolution[],
  ) {
    const activeView = context.workspaceShell.activeView()
    const workspace = get(context.workspaceSession).workspace
    const workspaceView = workspace
      ? sessionViewDescriptor(workspace.request.host_id, workspace.request.host_session_id)
      : null
    const safeResolutions = preserveLoadedSessionDuringUnconfirmedRecovery(
      resolutionsToApply,
      workspaceView,
    )
    const nextActive = sessionViewResolution(safeResolutions, context.workspaceShell.activeViewKey())
    if (
      activeView?.kind === 'session' &&
      nextActive?.kind === 'missing-session' &&
      workspaceView &&
      workspaceViewKey(workspaceView) === workspaceViewKey(activeView)
    ) {
      const recoveryTransition = {}
      activeRecoveryTransition = recoveryTransition
      const outcome = await context.workspaceTransition.activate({
        view: activeView,
        requestId: null,
        shellAction: { type: 'open' },
        pendingViewKey: workspaceViewKey(activeView),
      })
      if (activeRecoveryTransition === recoveryTransition) activeRecoveryTransition = null
      if (outcome !== 'activated') return false
      await context.navigation.selectScope(null, null)
    }
    sessionViewResolutions = safeResolutions
    return true
  }

  function start(): Promise<boolean> {
    if (ready) return ready
    patch({ phase: 'loading', failureMessage: '', failureTimedOut: false, settingsOpen: false, workspaceFailure: null })
    const finish = startClientDiagnostic('application_startup', {
      source: 'workbench',
      phase: 'initialization',
    })
    const run = (async () => {
      const initialized = await context.navigation.initialize(
        !context.workspaceShell.hasRestoredSnapshot(),
      )
      const navigation = get(context.navigation)
      if (!initialized) {
        const failure = get(store).workspaceFailure
        patch({
          failureMessage: failure
            ? context.messageFrom(failure)
            : navigation.initializationFailure?.message ||
              context.pageError() ||
              context.tr('Could not load the workbench.'),
          failureTimedOut:
            failure instanceof ApplicationReadTimeoutError ||
            (navigation.initializationFailure?.timedOut ?? false),
          phase: 'failed',
        })
        finish('failed', {
          reason: get(store).failureTimedOut ? 'timeout' : 'initialization_failed',
        })
        return false
      }
      await refreshSessionViewRecovery()
      if (context.workspaceShell.hasRestoredSnapshot()) {
        await restoreInitialWorkspaceSnapshot(true)
      } else if (context.previewMode) {
        const request = get(context.workspaceSession).workspace?.request
        if (request) await context.navigation.selectScope(request.host_id, request.host_session_id)
      }
      patch({ phase: 'ready' })
      context.onReady()
      finish('ok')
      return true
    })().catch((cause) => {
      finish('failed', { error_category: diagnosticErrorCategory(cause) })
      const message = context.messageFrom(cause)
      patch({
        failureMessage: message,
        failureTimedOut: cause instanceof ApplicationReadTimeoutError,
        phase: 'failed',
      })
      context.setPageError(message)
      return false
    })
    ready = run
    void run.then((initialized) => {
      if (!initialized && ready === run) ready = null
    })
    return run
  }

  async function retrySessionViewRecovery() {
    const retryingMissingView = resolutionFor(context.workspaceShell.activeViewKey())?.kind === 'missing-session'
    if (retryingMissingView) {
      patch({ mounted: false })
      context.workspaceSession.setLoading(true)
    }
    await context.navigation.refreshNavigation(true)
    resetRecoveryFingerprint()
    await refreshSessionViewRecovery()
    const activeResolution = resolutionFor(context.workspaceShell.activeViewKey())
    if (activeResolution?.kind === 'active' && get(context.workspaceSession).workspace === null) {
      await restoreInitialWorkspaceSnapshot()
    } else if (retryingMissingView) {
      patch({ mounted: true })
      context.workspaceSession.setLoading(false)
    }
  }

  /** Re-runs recovery when the caller's fingerprint of catalog plus open views changed. */
  async function recoverIfChanged(fingerprint: string) {
    if (get(context.navigation).hostSessionFactsStatus === 'pending') return
    if (fingerprint === lastRecoveryFingerprint) return
    lastRecoveryFingerprint = fingerprint
    await refreshSessionViewRecovery()
  }

  function resetRecoveryFingerprint() {
    lastRecoveryFingerprint = ''
  }

  async function restoreInitialWorkspaceSnapshot(throwOnFailure = false) {
    const view = context.workspaceShell.activeView()
    if (!view) {
      context.clearWorkspace()
      patch({ mounted: true })
      context.workspaceSession.setLoading(false)
      return
    }
    if (view.kind === 'agent-draft') {
      context.clearWorkspace()
      context.workspaceSession.setLoading(false)
      patch({ mounted: true })
      return
    }
    if (view.kind === 'agent-session') {
      await context.selectAgentNavigationScope(view)
      const navigation = get(context.navigation)
      const session = agentSessionForView(view, navigation.hostSessions)
      if (session && session.request_count > 0 && session.pending_count === 0) {
        // Use an unfiltered local read: a saved search can otherwise conceal the
        // cancellation and mounting the restored Agent view would launch ACP.
        const result = await readApplicationSnapshot(context.transport, 'listFeedbackRequests', {
          host_id: session.host_id,
          host_session_id: session.host_session_id,
          status: null,
          archived: null,
          search: null,
          limit: 1,
          cursor: null,
        })
        const target = cancelledFeedbackRestoreTarget(session, result.requests)
        if (target) {
          const outcome = await context.workspaceTransition.activate({
            ...target,
            shellAction: { type: 'open' },
            pendingViewKey: workspaceViewKey(target.view),
          })
          if (throwOnFailure && outcome === 'failed') {
            throw (
              get(store).workspaceFailure ??
              new Error(context.pageError() || context.tr('The initial workspace could not be opened.'))
            )
          }
          return
        }
      }
      context.clearWorkspace()
      patch({ mounted: true })
      context.workspaceSession.setLoading(false)
      return
    }
    if (
      view.kind === 'inbox' ||
      view.kind === 'archive' ||
      view.kind === 'settings' ||
      view.kind === 'rambelle-profile'
    ) {
      context.clearWorkspace()
      patch({ mounted: true })
      context.workspaceSession.setLoading(false)
      if (view.kind === 'inbox') await context.navigation.selectScope(null, null)
      return
    }
    if (view.kind === 'request-task') {
      const outcome = await context.workspaceTransition.activate({
        view,
        requestId: view.requestId,
        shellAction: { type: 'open' },
        pendingViewKey: workspaceViewKey(view),
      })
      if (throwOnFailure && outcome === 'failed') {
        throw (
          get(store).workspaceFailure ??
          new Error(context.pageError() || context.tr('The initial workspace could not be opened.'))
        )
      }
      return
    }

    const resolution = resolutionFor(workspaceViewKey(view))
    if (!resolution || resolution.kind !== 'active') {
      context.clearWorkspace()
      patch({ mounted: true })
      context.workspaceSession.setLoading(false)
      return
    }

    const selection = await context.navigation.selectScope(view.hostId, view.hostSessionId)
    if (!selection.selected) {
      patch({ mounted: true })
      context.workspaceSession.setLoading(false)
      if (throwOnFailure) {
        throw new Error(
          context.pageError() || context.tr('The initial workspace could not be opened.'),
        )
      }
      return
    }
    const outcome = await context.workspaceTransition.activate({
      view,
      requestId: context.requestIdForSession(view, selection.requests),
      shellAction: { type: 'open' },
      pendingViewKey: workspaceViewKey(view),
    })
    if (throwOnFailure && outcome === 'failed') {
      throw (
        get(store).workspaceFailure ??
        new Error(context.pageError() || context.tr('The initial workspace could not be opened.'))
      )
    }
  }

  return {
    subscribe: store.subscribe,
    patch,
    start,
    refreshSessionViewRecovery,
    retrySessionViewRecovery,
    recoverIfChanged,
    resetRecoveryFingerprint,
    restoreInitialWorkspaceSnapshot,
    applySessionViewResolutions,
    resolutionFor,
    resolutions,
    phase,
  }
}

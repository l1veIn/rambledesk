import { get, writable } from 'svelte/store'

import { ApplicationReadTimeoutError, withApplicationReadTimeout } from '../application/applicationReadTimeout'
import { readApplicationSnapshot } from '../application/readApplicationSnapshot'
import type { ApplicationTransport } from '../application/applicationTransport'
import { startClientDiagnostic, diagnosticErrorCategory } from '../diagnostics/clientDiagnostics'
import { agentSessionForView, cancelledFeedbackRestoreTarget } from '../workspace/agentViewRouting'
import {
  createSessionViewRecoveryResolver,
  preserveLoadedSessionDuringUnconfirmedRecovery,
  sessionViewResolution,
  type SessionViewCatalog,
  type SessionViewResolution,
} from '../workspace/sessionViewRecovery'
import { sessionViewDescriptor, workspaceViewKey } from '../workspace/viewDescriptors'
import type { createNavigationController } from './navigationController'
import type { WorkspaceSession } from './workspaceSession'
import type { WorkspaceShellSession } from './workspaceShellSession'
import type { WorkspaceNavigationController } from './workspaceNavigationController'

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
  resolutions: readonly SessionViewResolution[]
}>

type NavigationController = ReturnType<typeof createNavigationController>

export type StartupControllerContext = {
  navigation: NavigationController
  workspaceShell: WorkspaceShellSession
  workspaceSession: WorkspaceSession
  transport: ApplicationTransport
  workspaceNavigation: () => Pick<WorkspaceNavigationController, 'activateView' | 'invalidate' | 'currentIntent' | 'isCurrent' | 'clearWorkspace'>
  tr: (source: string, values?: Record<string, string | number>) => string
  messageFrom: (cause: unknown) => string
  pageError: () => string
  setPageError: (message: string) => void
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
  resolutions: [],
}

export type StartupController = ReturnType<typeof createStartupController>

export function createStartupController(context: StartupControllerContext) {
  let disposed = false
  // A restored view mounts only after its workspace loads.
  const store = writable<StartupState>({
    ...initial,
    mounted: !context.workspaceShell.restoredActiveView(),
  })
  let ready: Promise<boolean> | null = null
  let lastRecoveryFingerprint = ''
  let activeRecoveryTransition: { intent: number } | null = null

  function patch(next: Partial<StartupState>) {
    if (disposed) return
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
      const sessions = await context.transport.call('listArchivedHostSessions', { search: null })
      return sessions.map((session) =>
        sessionViewDescriptor(session.host_id, session.host_session_id),
      )
    },
    onInvalidate: () => {
      if (disposed || !activeRecoveryTransition) return
      const recovery = activeRecoveryTransition
      activeRecoveryTransition = null
      if (context.workspaceNavigation().isCurrent(recovery.intent)) context.workspaceNavigation().invalidate()
    },
    onUpdate: applySessionViewResolutions,
  })

  async function refreshSessionViewRecovery() {
    if (disposed) return 'stale' as const
    return recoveryResolver.refresh(
      context.workspaceShell.views().filter(
        (view): view is Extract<typeof view, { kind: 'session' }> => view.kind === 'session',
      ),
      activeSessionCatalog(),
    )
  }

  function resolutionFor(viewKey: string | null): SessionViewResolution | null {
    return sessionViewResolution(get(store).resolutions, viewKey)
  }

  function phase() {
    return get(store).phase
  }

  function resolutions(): readonly SessionViewResolution[] {
    return get(store).resolutions
  }

  async function applySessionViewResolutions(
    resolutionsToApply: readonly SessionViewResolution[],
  ) {
    if (disposed) return false
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
      const recoveryTransition = { intent: context.workspaceNavigation().currentIntent() }
      activeRecoveryTransition = recoveryTransition
      const outcome = await context.workspaceNavigation().activateView(activeView, {
        requestId: null, missingSession: true, expectedIntent: recoveryTransition.intent,
      })
      if (activeRecoveryTransition === recoveryTransition) activeRecoveryTransition = null
      if (outcome !== 'activated') return false
    }
    patch({ resolutions: safeResolutions })
    return true
  }

  function start(): Promise<boolean> {
    if (disposed) return Promise.resolve(false)
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
      if (disposed) return false
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
      if (disposed) return false
      if (context.workspaceShell.hasRestoredSnapshot()) {
        await restoreInitialWorkspaceSnapshot(true)
      }
      if (disposed) return false
      patch({ phase: 'ready' })
      context.onReady()
      finish('ok')
      return true
    })().catch((cause) => {
      if (disposed) return false
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
    if (disposed) return
    const intent = context.workspaceNavigation().currentIntent()
    const retryingMissingView = resolutionFor(context.workspaceShell.activeViewKey())?.kind === 'missing-session'
    if (retryingMissingView) {
      patch({ mounted: false })
      context.workspaceSession.setLoading(true)
    }
    await context.navigation.refreshNavigation(true)
    if (disposed || !context.workspaceNavigation().isCurrent(intent)) return
    resetRecoveryFingerprint()
    await refreshSessionViewRecovery()
    if (disposed || !context.workspaceNavigation().isCurrent(intent)) return
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
    if (disposed) return
    if (get(context.navigation).hostSessionFactsStatus === 'pending') return
    if (fingerprint === lastRecoveryFingerprint) return
    lastRecoveryFingerprint = fingerprint
    await refreshSessionViewRecovery()
  }

  function resetRecoveryFingerprint() {
    lastRecoveryFingerprint = ''
  }

  async function restoreInitialWorkspaceSnapshot(throwOnFailure = false) {
    if (disposed) return
    const navigation = context.workspaceNavigation()
    const intent = navigation.currentIntent()
    const view = context.workspaceShell.activeView()
    if (!view) {
      navigation.clearWorkspace()
      patch({ mounted: true })
      context.workspaceSession.setLoading(false)
      return
    }
    let target = view
    let requestId: string | undefined
    if (view.kind === 'agent-session') {
      const session = agentSessionForView(view, get(context.navigation).hostSessions)
      if (session && session.request_count > 0 && session.pending_count === 0) {
        // Saved filters cannot conceal a cancellation and accidentally launch ACP.
        const result = await readApplicationSnapshot(context.transport, 'listFeedbackRequests', {
          host_id: session.host_id, host_session_id: session.host_session_id,
          status: null, archived: null, search: null, limit: 1, cursor: null,
        })
        if (disposed || !navigation.isCurrent(intent)) return
        const cancelled = cancelledFeedbackRestoreTarget(session, result.requests)
        if (cancelled) {
          target = cancelled.view
          requestId = cancelled.requestId
        }
      }
    }
    const missingSession = requestId === undefined && target.kind === 'session' && resolutionFor(workspaceViewKey(target))?.kind !== 'active'
    const outcome = await navigation.activateView(target, {
      ...(requestId ? { requestId } : {}), expectedIntent: intent, missingSession,
    })
    if (throwOnFailure && outcome === 'failed') {
      throw get(store).workspaceFailure ?? new Error(context.pageError() || context.tr('The initial workspace could not be opened.'))
    }
  }

  function dispose() {
    disposed = true
    recoveryResolver.invalidate()
    activeRecoveryTransition = null
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
    dispose,
  }
}

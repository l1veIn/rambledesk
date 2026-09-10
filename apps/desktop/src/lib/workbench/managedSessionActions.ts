import { get, writable } from 'svelte/store'

import {
  createDraftManagedSessionController,
  type DraftManagedSessionController,
} from '../agents/draftManagedSessionController'
import { createManagedSessionDraftStorage } from '../agents/managedSessionDrafts'
import { removeArchivedManagedSessionView } from '../agents/managedSessionArchive'
import { deleteSessionRecord, removeManagedSessionViews } from '../agents/managedSessionDeletion'
import { startClientDiagnostic, diagnosticErrorCategory } from '../diagnostics/clientDiagnostics'
import type { ApplicationTransport } from '../application/applicationTransport'
import type { HostSessionSummary } from '../feedback'
import type { ManagedSessionSnapshot } from '../generated/feedback'
import {
  agentDraftViewDescriptor,
  agentSessionViewDescriptor,
  inboxViewDescriptor,
  sessionViewDescriptor,
  workspaceViewKey,
} from '../workspace/viewDescriptors'
import type { createNavigationController } from './navigationController'

type NavigationController = ReturnType<typeof createNavigationController>
import type { WorkspaceSession } from './workspaceSession'
import type { WorkspaceShellSession } from './workspaceShellSession'
import type { WorkspaceNavigationController } from './workspaceNavigationController'

/**
 * Managed Agent sessions: opening a session or draft, promoting a draft into a
 * session, and archiving or deleting sessions from the workbench.
 *
 * Owns the draft-controller cache and the per-session pending flags; the workbench
 * renders them and the navigation controller closes drafts through this module.
 */
export type ManagedSessionActionsState = Readonly<{
  deletingCommands: ReadonlySet<string>
  deletingSessions: ReadonlySet<string>
}>

export type ManagedSessionActionsContext = {
  transport: ApplicationTransport
  navigation: NavigationController
  workspaceShell: WorkspaceShellSession
  workspaceSession: WorkspaceSession
  workspaceNavigation: () => Pick<WorkspaceNavigationController, 'activateView' | 'invalidate' | 'currentIntent' | 'isCurrent' | 'clearWorkspace'>
  tr: (source: string, values?: Record<string, string | number>) => string
  messageFrom: (cause: unknown) => string
  setPageError: (message: string) => void
  isTransitionLocked: () => boolean
  /** Whether this client can host a managed Agent session at all. */
  canOpenManagedSession: () => boolean
  exitRamble: () => Promise<void>
  rambleCanExit: () => boolean
  openArchivedSessions: (initial: ReturnType<typeof sessionViewDescriptor>) => Promise<void>
}

export type ManagedSessionActions = ReturnType<typeof createManagedSessionActions>

export function createManagedSessionActions(context: ManagedSessionActionsContext) {
  const store = writable<ManagedSessionActionsState>({
    deletingCommands: new Set(),
    deletingSessions: new Set(),
  })
  const draftStorage = createManagedSessionDraftStorage(
    typeof localStorage === 'undefined' ? undefined : localStorage,
  )
  const draftControllers = new Map<string, DraftManagedSessionController>()
  const promotedDrafts = new Map<string, string>()
  const closingDrafts = new Set<string>()

  function patch(next: Partial<ManagedSessionActionsState>) {
    store.update((current) => ({ ...current, ...next }))
  }

  function setDeletingCommands(sessionId: string, deleting: boolean) {
    const next = new Set(get(store).deletingCommands)
    if (deleting) next.add(sessionId)
    else next.delete(sessionId)
    patch({ deletingCommands: next })
  }

  function observeDeletion(sessionId: string, deleting: boolean) {
    const next = new Set(get(store).deletingSessions)
    if (deleting) next.add(sessionId)
    else next.delete(sessionId)
    patch({ deletingSessions: next })
  }

  function draftController(draftId: string): DraftManagedSessionController {
    let controller = draftControllers.get(draftId)
    if (!controller) {
      controller = createDraftManagedSessionController(
        context.transport,
        draftId,
        draftStorage,
        (snapshot) => void draftPromoted(draftId, snapshot),
      )
      draftControllers.set(draftId, controller)
    }
    return controller
  }

  /** Closes a draft, promoting it into a session when the agent accepted the work. */
  async function closeDraft(
    draftId: string,
  ): Promise<Readonly<{ skipped: boolean; promotedSessionId: string | null }>> {
    if (closingDrafts.has(draftId)) return { skipped: true, promotedSessionId: null }
    closingDrafts.add(draftId)
    try {
      const promotedSessionId = await draftController(draftId).close()
      draftControllers.delete(draftId)
      return { skipped: false, promotedSessionId }
    } finally {
      closingDrafts.delete(draftId)
    }
  }

  async function draftPromoted(draftId: string, snapshot: ManagedSessionSnapshot) {
    promotedDrafts.set(draftId, snapshot.session.session_id)
    const view = agentSessionViewDescriptor(snapshot.session.session_id)
    context.workspaceShell.dispatch({
      type: 'replace',
      viewKey: workspaceViewKey(agentDraftViewDescriptor(draftId)),
      view,
    })
    draftControllers.delete(draftId)
    const intent = context.workspaceNavigation().currentIntent()
    await context.navigation.refreshNavigation(true)
    if (
      context.workspaceNavigation().isCurrent(intent) &&
      get(context.workspaceShell).shell.activeViewKey === workspaceViewKey(view)
    ) {
      await context.workspaceNavigation().activateView(view, { expectedIntent: intent })
    }
  }

  async function openAgentSession(sessionId: string) {
    if (context.isTransitionLocked()) return
    await context.workspaceNavigation().activateView(agentSessionViewDescriptor(sessionId))
  }

  async function openNewManagedSession(configId?: string, cwd = ''): Promise<boolean> {
    const finish = startClientDiagnostic('session_navigation', {
      action: 'open',
      source: context.tr('workbench'),
      selected: !!configId,
    })
    if (context.isTransitionLocked() || !context.canOpenManagedSession()) {
      finish('blocked', { reason: context.canOpenManagedSession() ? 'in_flight' : 'unsupported' })
      return false
    }
    try {
      const view = agentDraftViewDescriptor(crypto.randomUUID())
      const recent = draftStorage.load(view.draftId)
      draftStorage.save(view.draftId, {
        choice: configId ? `config:${configId}` : recent.choice,
        cwd,
        text: '',
      })
      const outcome = await context.workspaceNavigation().activateView(view)
      if (outcome !== 'activated') draftStorage.remove(view.draftId)
      finish(
        outcome === 'activated'
          ? 'ok'
          : outcome === 'failed'
            ? 'failed'
            : outcome === 'stale'
              ? 'cancelled'
              : 'blocked',
        outcome === 'activated'
          ? {}
          : {
              reason:
                outcome === 'stale'
                  ? 'stale'
                  : outcome === 'blocked'
                    ? 'in_flight'
                    : 'activation_failed',
            },
      )
      return outcome === 'activated'
    } catch (cause) {
      finish('failed', { error_category: diagnosticErrorCategory(cause) })
      throw cause
    }
  }

  async function archiveSessionFromUi(session: HostSessionSummary): Promise<void> {
    const finish = startClientDiagnostic('session_archive', {
      source: 'workbench',
      action: 'archive',
      management: session.management.kind,
    })
    try {
      if (!(await context.navigation.archiveHostSession(session))) {
        finish('blocked', { reason: 'not_ready' })
        return
      }
      if (session.management.kind !== 'managed') {
        finish('ok')
        return
      }
      const key = workspaceViewKey(agentSessionViewDescriptor(session.session_id))
      const shell = get(context.workspaceShell).shell
      const archived = removeArchivedManagedSessionView(
        shell,
        session.session_id,
        get(context.workspaceShell).pendingViewKey,
      )
      if (archived.shouldInvalidatePending) context.workspaceNavigation().invalidate()
      context.workspaceShell.replaceShell(archived.shell)
      context.workspaceShell.forgetRequest(key)
      if (archived.shouldNavigateToArchive) {
        await context.openArchivedSessions(
          sessionViewDescriptor(session.host_id, session.host_session_id),
        )
      }
      finish('ok')
    } catch (cause) {
      finish('failed', { error_category: diagnosticErrorCategory(cause) })
      throw cause
    }
  }

  async function deleteManagedSessionFromUi(session: HostSessionSummary) {
    if (
      session.management.kind !== 'managed' ||
      get(store).deletingCommands.has(session.session_id)
    ) {
      return
    }
    const finish = startClientDiagnostic('session_delete', {
      source: 'workbench',
      action: 'delete',
      management: 'managed',
    })
    setDeletingCommands(session.session_id, true)
    try {
      const workspaceRequest = get(context.workspaceSession).workspace?.request
      const ownsFeedback = () => workspaceRequest?.managed_session_id === session.session_id
      if (ownsFeedback() && context.rambleCanExit()) await context.exitRamble()
      await deleteSessionRecord(context.transport, session)
      const viewKey = workspaceViewKey(
        sessionViewDescriptor(session.host_id, session.host_session_id),
      )
      const requestId = ownsFeedback() ? (workspaceRequest?.request_id ?? null) : null
      const rememberedRequestId = context.workspaceShell.requestIdFor(viewKey)
      const navigation = get(context.navigation)
      const knownRequestIds = [
        ...new Set(
          [
            requestId,
            rememberedRequestId,
            ...[...navigation.requests, ...navigation.pendingRequests]
              .filter((request) => request.managed_session_id === session.session_id)
              .map((request) => request.request_id),
          ].filter((id): id is string => !!id),
        ),
      ]
      const cleanup = removeManagedSessionViews(get(context.workspaceShell).shell, session, knownRequestIds)
      const closedActive = cleanup.closedActive || ownsFeedback()
      const pendingViewKey = get(context.workspaceShell).pendingViewKey
      if (pendingViewKey && cleanup.closedViewKeys.includes(pendingViewKey)) {
        context.workspaceNavigation().invalidate()
      }
      if (closedActive) {
        context.workspaceNavigation().invalidate()
        context.workspaceNavigation().clearWorkspace()
      }
      context.workspaceShell.replaceShell(cleanup.shell)
      if (closedActive) context.workspaceShell.dispatch({ type: 'open', view: inboxViewDescriptor() })
      context.workspaceShell.forgetRequest(viewKey)
      observeDeletion(session.session_id, false)
      if (
        closedActive ||
        (navigation.selectedHostId === session.host_id &&
          navigation.selectedHostSessionId === session.host_session_id)
      ) {
        await context.navigation.selectScope(null, null)
      }
      await context.navigation.refreshNavigation(true)
      finish('ok')
    } catch (cause) {
      finish('failed', { error_category: diagnosticErrorCategory(cause) })
      context.setPageError(context.messageFrom(cause))
      throw cause
    } finally {
      setDeletingCommands(session.session_id, false)
    }
  }

  /** Closes every cached draft controller, used when the workbench unmounts. */
  function closeAllDrafts() {
    for (const controller of draftControllers.values()) void controller.close().catch(() => {})
    draftControllers.clear()
  }

  return {
    subscribe: store.subscribe,
    closeAllDrafts,
    observeDeletion,
    draftController,
    closeDraft,
    openAgentSession,
    openNewManagedSession,
    archiveSessionFromUi,
    deleteManagedSessionFromUi,
    promotedSessionId: (draftId: string) => promotedDrafts.get(draftId),
  }
}

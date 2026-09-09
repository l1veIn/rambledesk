import { get, writable } from 'svelte/store'

import type { ApplicationTransport } from '$lib/application/applicationTransport'
import type { FeedbackRequestSummary, FeedbackWorkspaceView, HostSessionSummary } from '$lib/feedback'
import { normalizePublishedFeedback, type PublishedFeedbackView } from '$lib/publishedFeedback'
import { deleteSessionRecord } from '$lib/agents/managedSessionDeletion'
import {
  ALL_ARCHIVE_REQUEST_STATUSES,
  archivedSessionKey,
} from './archiveSearch'

export type ArchivedRequestDetails = {
  workspace: FeedbackWorkspaceView
  publishedFeedback: PublishedFeedbackView | null
}

export type ArchiveActionsState = Readonly<{
  loading: boolean
  busyKey: string | null
  sessions: HostSessionSummary[]
  requestsBySession: Record<string, FeedbackRequestSummary[]>
  requestDetailsById: Record<string, ArchivedRequestDetails>
  detailLoadingRequestId: string | null
}>

export type ArchiveLoadResult = Readonly<{
  sessions: HostSessionSummary[]
  requestsBySession: Record<string, FeedbackRequestSummary[]>
}>

export type ArchiveActionsContext = {
  transport: ApplicationTransport
  tr: (source: string, values?: Record<string, string | number>) => string
  messageFrom: (cause: unknown) => string
  onError: (message: string) => void
  onChanged: () => Promise<void> | void
  onDeleteManagedSession?: (session: HostSessionSummary) => Promise<void> | void
  /** Re-runs the view's selection logic after a mutation. */
  reload: () => Promise<void>
}

const initial: ArchiveActionsState = {
  loading: false,
  busyKey: null,
  sessions: [],
  requestsBySession: {},
  requestDetailsById: {},
  detailLoadingRequestId: null,
}

/**
 * Archived-session data: loading the archive, lazily loading request details, and
 * the unarchive/delete mutations with their busy and reload cycle. The view keeps
 * the search text and the selection.
 */
export function createArchiveActions(context: ArchiveActionsContext) {
  const store = writable<ArchiveActionsState>(initial)

  function patch(next: Partial<ArchiveActionsState>) {
    store.update((current) => ({ ...current, ...next }))
  }

  async function fetchSessionRequests(session: HostSessionSummary, search: string) {
    return (
      await context.transport.call('listFeedbackRequests', {
        host_id: session.host_id,
        host_session_id: session.host_session_id,
        status: [...ALL_ARCHIVE_REQUEST_STATUSES],
        archived: true,
        search: search.trim() || null,
        limit: 100,
        cursor: null,
      })
    ).requests
  }

  async function load(search: string): Promise<ArchiveLoadResult | null> {
    patch({ loading: true })
    try {
      const nextSessions = await context.transport.call('listArchivedHostSessions', {
        search: search.trim() || null,
      })
      const entries = await Promise.all(
        nextSessions.map(
          async (session) =>
            [archivedSessionKey(session), await fetchSessionRequests(session, search)] as const,
        ),
      )
      const requestsBySession = Object.fromEntries(entries)
      patch({ sessions: nextSessions, requestsBySession })
      return { sessions: nextSessions, requestsBySession }
    } catch (cause) {
      context.onError(context.messageFrom(cause))
      return null
    } finally {
      patch({ loading: false })
    }
  }

  async function loadRequestDetails(request: FeedbackRequestSummary) {
    const state = get(store)
    if (state.requestDetailsById[request.request_id] || state.detailLoadingRequestId === request.request_id) {
      return
    }
    patch({ detailLoadingRequestId: request.request_id })
    try {
      const workspace = await context.transport.call('getFeedbackWorkspace', {
        request_id: request.request_id,
      })
      if (!workspace) throw new Error(context.tr('This feedback request could not be found.'))
      const publishedFeedback =
        workspace.request.status === 'completed' && workspace.feedback
          ? normalizePublishedFeedback(
              await context.transport.call('readPublishedFeedback', {
                request_id: request.request_id,
              }),
            )
          : null
      patch({
        requestDetailsById: {
          ...get(store).requestDetailsById,
          [request.request_id]: { workspace, publishedFeedback },
        },
      })
    } catch (cause) {
      context.onError(context.messageFrom(cause))
    } finally {
      if (get(store).detailLoadingRequestId === request.request_id) {
        patch({ detailLoadingRequestId: null })
      }
    }
  }

  async function runAction(key: string, action: () => Promise<void> | void) {
    if (get(store).busyKey) return
    patch({ busyKey: key })
    try {
      await action()
      await context.onChanged()
      await context.reload()
    } catch (cause) {
      context.onError(context.messageFrom(cause))
    } finally {
      patch({ busyKey: null })
    }
  }

  async function unarchiveSession(session: HostSessionSummary) {
    await runAction(`unarchive:${session.host_id}:${session.host_session_id}`, async () => {
      await context.transport.call('unarchiveHostSession', {
        host_id: session.host_id,
        host_session_id: session.host_session_id,
      })
    })
  }

  async function deleteSession(session: HostSessionSummary) {
    if (
      session.management.kind !== 'managed' &&
      !confirm(context.tr('Delete this archived session permanently?'))
    ) {
      return
    }
    await runAction(`delete-session:${session.host_id}:${session.host_session_id}`, async () => {
      if (session.management.kind === 'managed') {
        if (context.onDeleteManagedSession) await context.onDeleteManagedSession(session)
        else await deleteSessionRecord(context.transport, session)
        return
      }
      await context.transport.call('deleteHostSession', {
        host_id: session.host_id,
        host_session_id: session.host_session_id,
      })
    })
  }

  async function deleteRequest(request: FeedbackRequestSummary) {
    if (!confirm(context.tr('Delete this archived request permanently?'))) return
    await runAction(`delete-request:${request.request_id}`, async () => {
      await context.transport.call('deleteFeedbackRequest', { request_id: request.request_id })
    })
  }

  return {
    subscribe: store.subscribe,
    load,
    loadRequestDetails,
    unarchiveSession,
    deleteSession,
    deleteRequest,
    state: () => get(store),
  }
}

export type ArchiveActions = ReturnType<typeof createArchiveActions>

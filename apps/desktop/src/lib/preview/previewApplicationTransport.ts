import type {
  ApplicationCommandInput,
  ApplicationCommandName,
  ApplicationCommandResult,
} from '../application/contracts'
import type {
  ApplicationStream,
  ApplicationTransport,
  SubscriptionErrorHandler,
  Unsubscribe,
} from '../application/applicationTransport'
import type { CapabilityManifest } from '../capabilities/capabilityManifest'
import {
  filterRequestPage,
  requestFilterStatuses,
  requestMatchesSearch,
} from '../domain/requestFilters'
import type {
  AttachmentView,
  DraftView,
  FeedbackRequestSummary,
  FeedbackWorkspaceView,
  HostSessionSummary,
  SaveDraftInput,
} from '../feedback'
import { previewFixtures, previewWorkspaceFor } from './previewFixtures'

/**
 * Fixture-backed ApplicationTransport for the `?preview=fixtures` workbench.
 *
 * It behaves like a small in-memory server so production controllers never need
 * a `previewMode` branch: reads return fixtures, mutations update them, and the
 * normal query/invalidate flow runs unchanged.
 */
export class PreviewApplicationTransport implements ApplicationTransport {
  readonly #requests = new Map<string, FeedbackRequestSummary>()
  readonly #hostSessions = new Map<string, HostSessionSummary>()
  readonly #archivedSessions = new Map<string, HostSessionSummary>()
  readonly #drafts = new Map<string, DraftView>()
  readonly #attachments = new Map<string, AttachmentView[]>()
  readonly #submitted = new Set<string>()

  constructor(private readonly capabilityManifest: CapabilityManifest) {
    for (const request of previewFixtures.requests) this.#requests.set(request.request_id, { ...request })
    for (const session of previewFixtures.hostSessions) this.#hostSessions.set(session.session_id, { ...session })
    for (const session of previewFixtures.archivedHostSessions) {
      this.#archivedSessions.set(session.session_id, { ...session })
    }
  }

  call<Name extends ApplicationCommandName>(
    name: Name,
    input: ApplicationCommandInput<Name>,
  ): Promise<ApplicationCommandResult<Name>> {
    try {
      return Promise.resolve(this.#dispatch(name, input as never) as ApplicationCommandResult<Name>)
    } catch (cause) {
      return Promise.reject(cause)
    }
  }

  subscribe<Event>(
    _stream: ApplicationStream<Event>,
    _handler: (event: Event) => void,
    _onError: SubscriptionErrorHandler,
  ): Unsubscribe {
    // Fixtures never change behind the caller's back, so there is nothing to emit.
    return () => {}
  }

  waitUntilReady(): Promise<void> {
    return Promise.resolve()
  }

  capabilities(): CapabilityManifest {
    return this.capabilityManifest
  }

  #dispatch(name: ApplicationCommandName, input: unknown): unknown {
    switch (name) {
      case 'listFeedbackInbox':
        return [...this.#requests.values()].filter(
          (request) => request.status === 'waiting' || request.status === 'in_progress',
        )
      case 'listHostSessions':
        return [...this.#hostSessions.values()]
      case 'listArchivedHostSessions': {
        const search = (input as { search: string | null } | undefined)?.search ?? null
        const normalized = search?.trim().toLowerCase() ?? ''
        return [...this.#archivedSessions.values()].filter(
          (session) =>
            !normalized ||
            [session.title, session.source_hint, session.host_id, session.host_session_id].some(
              (value) => (value ?? '').toLowerCase().includes(normalized),
            ),
        )
      }
      case 'listHostProfiles':
        return previewFixtures.hostProfiles
      case 'listFeedbackRequests':
        return this.#listRequests(input as {
          host_id: string | null
          host_session_id: string | null
          status: FeedbackRequestSummary['status'][]
          archived: boolean | null
          search: string | null
          limit: number | null
          cursor: string | null
        })
      case 'getFeedbackWorkspace':
        return this.#workspaceFor((input as { request_id: string }).request_id)
      case 'readPublishedFeedback': {
        const workspace = this.#workspaceFor((input as { request_id: string }).request_id)
        return workspace.feedback
          ? {
              manifest: {
                schema_version: 1,
                request_id: workspace.request.request_id,
                title: workspace.request.title,
                host_id: workspace.request.host_id,
                host_session_id: workspace.request.host_session_id,
                source_hint: workspace.request.source_hint,
                submitted_at: workspace.request.updated_at,
                source_revision: workspace.request.revision,
                draft_revision: workspace.draft.saved_revision,
                feedback_markdown: workspace.draft.body_markdown,
                feedback_sha256: 'preview',
                attachments: [],
              },
              markdown: workspace.draft.body_markdown,
              uncooked_markdown: workspace.draft.body_markdown,
            }
          : null
      }
      case 'saveFeedbackDraft':
        return this.#saveDraft(input as SaveDraftInput)
      case 'submitFeedback': {
        const requestId = (input as { request_id: string }).request_id
        const request = this.#requests.get(requestId)
        if (!request) throw new Error('This feedback request could not be found.')
        const submitted: FeedbackRequestSummary = {
          ...request,
          status: 'completed',
          resolution: 'feedback_submitted',
          revision: request.revision + 1,
          updated_at: new Date().toISOString(),
        }
        this.#requests.set(requestId, submitted)
        this.#submitted.add(requestId)
        return { ...submitted, feedback: { available: true } }
      }
      case 'renameHostSession': {
        const { host_id, host_session_id, title } = input as {
          host_id: string
          host_session_id: string
          title: string
        }
        const session = this.#findSession(host_id, host_session_id)
        const renamed = { ...session, title }
        this.#replaceSession(renamed)
        return renamed
      }
      case 'setHostSessionPinned': {
        const { host_id, host_session_id, pinned } = input as {
          host_id: string
          host_session_id: string
          pinned: boolean
        }
        const session = this.#findSession(host_id, host_session_id)
        const updated = { ...session, pinned_at: pinned ? new Date().toISOString() : null }
        this.#replaceSession(updated)
        return updated
      }
      case 'setHostPinned': {
        const { host_id, pinned } = input as { host_id: string; pinned: boolean }
        const pinnedAt = pinned ? new Date().toISOString() : null
        const sessions = [...this.#hostSessions.values()].map((session) =>
          session.host_id === host_id ? { ...session, host_pinned_at: pinnedAt } : session,
        )
        for (const session of sessions) this.#replaceSession(session)
        return sessions
      }
      case 'archiveHostSession': {
        const { host_id, host_session_id } = input as {
          host_id: string
          host_session_id: string
        }
        const session = this.#findSession(host_id, host_session_id)
        const archived = { ...session, archived_at: new Date().toISOString() }
        this.#hostSessions.delete(session.session_id)
        this.#archivedSessions.set(archived.session_id, archived)
        return archived
      }
      case 'unarchiveHostSession': {
        const { host_id, host_session_id } = input as {
          host_id: string
          host_session_id: string
        }
        const session = [...this.#archivedSessions.values()].find(
          (candidate) =>
            candidate.host_id === host_id && candidate.host_session_id === host_session_id,
        )
        if (!session) throw new Error('This archived session could not be found.')
        const restored = { ...session, archived_at: null }
        this.#archivedSessions.delete(session.session_id)
        this.#hostSessions.set(restored.session_id, restored)
        return restored
      }
      case 'deleteHostSession': {
        const { host_id, host_session_id } = input as {
          host_id: string
          host_session_id: string
        }
        for (const sessions of [this.#hostSessions, this.#archivedSessions]) {
          for (const [key, session] of sessions) {
            if (session.host_id === host_id && session.host_session_id === host_session_id) {
              sessions.delete(key)
            }
          }
        }
        return undefined
      }
      case 'deleteFeedbackRequest': {
        this.#requests.delete((input as { request_id: string }).request_id)
        return undefined
      }
      case 'addFeedbackAttachment': {
        const { request_id, file_name, media_type, contents } = input as {
          request_id: string
          file_name: string
          media_type: string
          contents: ArrayBuffer
        }
        const workspace = this.#workspaceFor(request_id)
        const attachment: AttachmentView = {
          attachment_id: `preview-${workspace.attachments.length + 1}`,
          file_name,
          media_type,
          byte_size: contents.byteLength,
          sha256: 'preview',
          position: workspace.attachments.length,
        }
        this.#attachments.set(request_id, [...workspace.attachments, attachment])
        return this.#workspaceFor(request_id)
      }
      case 'removeFeedbackAttachment': {
        const { request_id, attachment_id } = input as {
          request_id: string
          attachment_id: string
        }
        const workspace = this.#workspaceFor(request_id)
        this.#attachments.set(
          request_id,
          workspace.attachments.filter((attachment) => attachment.attachment_id !== attachment_id),
        )
        return this.#workspaceFor(request_id)
      }
      case 'reorderFeedbackAttachments': {
        const { request_id, attachment_ids } = input as {
          request_id: string
          attachment_ids: string[]
        }
        const workspace = this.#workspaceFor(request_id)
        const reordered = attachment_ids
          .map((id, position) => {
            const attachment = workspace.attachments.find(
              (candidate) => candidate.attachment_id === id,
            )
            return attachment ? { ...attachment, position } : null
          })
          .filter((attachment): attachment is AttachmentView => attachment !== null)
        this.#attachments.set(request_id, reordered)
        return this.#workspaceFor(request_id)
      }
      default:
        throw new Error(`The preview transport does not implement ${name}.`)
    }
  }

  #findSession(hostId: string, hostSessionId: string): HostSessionSummary {
    const session = [...this.#hostSessions.values()].find(
      (candidate) =>
        candidate.host_id === hostId && candidate.host_session_id === hostSessionId,
    )
    if (!session) throw new Error('This host session could not be found.')
    return session
  }

  #replaceSession(session: HostSessionSummary) {
    this.#hostSessions.set(session.session_id, session)
  }

  #workspaceFor(requestId: string): FeedbackWorkspaceView {
    const base = previewWorkspaceFor(requestId)
    const request = this.#requests.get(requestId)
    if (!base || !request) throw new Error('This feedback request could not be found.')
    return {
      ...base,
      request,
      draft: this.#drafts.get(requestId) ?? base.draft,
      attachments: this.#attachments.get(requestId) ?? base.attachments,
      feedback: this.#submitted.has(requestId) ? { available: true } : base.feedback,
    }
  }

  #saveDraft(input: SaveDraftInput): DraftView {
    const draft: DraftView = {
      document_json: input.document_json,
      body_markdown: input.body_markdown,
      saved_revision: input.expected_revision + 1,
      updated_at: new Date().toISOString(),
    }
    this.#drafts.set(input.request_id, draft)
    return draft
  }

  #listRequests(input: {
    host_id: string | null
    host_session_id: string | null
    status: FeedbackRequestSummary['status'][]
    archived: boolean | null
    search: string | null
    limit: number | null
    cursor: string | null
  }) {
    const search = input.search ?? ''
    const statuses = new Set(input.status ?? requestFilterStatuses('all'))
    const archived = input.archived === true
    const requests = [...this.#requests.values()].filter((request) => {
      if (input.host_id && request.host_id !== input.host_id) return false
      if (input.host_session_id && request.host_session_id !== input.host_session_id) return false
      if (archived !== this.#isArchived(request)) return false
      if (!statuses.has(request.status)) return false
      return requestMatchesSearch(request, search)
    })
    const offset = input.cursor ? Number(input.cursor) : 0
    const limit = input.limit ?? 100
    const page = requests.slice(offset, offset + limit)
    const next = offset + limit < requests.length ? String(offset + limit) : null
    return filterRequestPage({ requests: page, next_cursor: next }, 'all')
  }

  #isArchived(request: FeedbackRequestSummary) {
    return [...this.#archivedSessions.values()].some(
      (session) =>
        session.host_id === request.host_id &&
        session.host_session_id === request.host_session_id,
    )
  }
}

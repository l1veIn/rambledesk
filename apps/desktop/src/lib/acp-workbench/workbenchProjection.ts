import type {
  FeedbackRequestSummary,
  FeedbackWorkspaceView,
  HostSessionSummary,
} from '$lib/feedback'
import type {
  RequestListAgentProfile,
  WorkbenchRequestListItem,
} from '$lib/components/navigation/requestListItem'
import { workbenchRequestKey } from '$lib/components/navigation/requestListItem'
import {
  sessionRailKey,
  type SessionOrigin,
  type SessionRailItem,
} from '$lib/components/navigation/sessionRailItem'
import type { HostProfile } from '$lib/workbench/types'

import type {
  AcpSessionSummary,
  AgentSummary,
  AttentionItem,
  FeedbackAttentionItem,
  DraftSnapshotV3,
} from './types'

const TERMINAL_ICON = `
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
    stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path d="m4 17 6-6-6-6"/><path d="M12 19h8"/>
  </svg>`

const BOT_ICON = `
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
    stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <rect width="18" height="10" x="3" y="11" rx="2"/><circle cx="12" cy="5" r="2"/>
    <path d="M12 7v4M8 16h.01M16 16h.01"/>
  </svg>`

export type UnifiedWorkbenchRequestListItem = WorkbenchRequestListItem

export type UnifiedWorkbenchFilter = {
  /** `null` is the All Requests scope. */
  sessionKey: string | null
  search: string
}

export type UnifiedWorkbenchProjectionInput = {
  adapterSessions: HostSessionSummary[]
  adapterRequests: FeedbackRequestSummary[]
  acpSessions: AcpSessionSummary[]
  attentionItems: AttentionItem[]
  agents: AgentSummary[]
  resolveHostProfile: (hostId: string) => HostProfile
  filter?: UnifiedWorkbenchFilter
}

export type UnifiedWorkbenchProjection = {
  sessions: SessionRailItem[]
  requests: UnifiedWorkbenchRequestListItem[]
}

/**
 * Read-only projection that lets Adapter facts and Managed ACP facts
 * share navigation without erasing their origin or changing either store.
 */
export function projectUnifiedWorkbench(
  input: UnifiedWorkbenchProjectionInput,
): UnifiedWorkbenchProjection {
  const sessions = [
    ...input.adapterSessions.map((session) => projectAdapterSession(
      session,
      input.resolveHostProfile(session.host_id),
    )),
    ...input.acpSessions.map((session) => projectManagedAcpSession(
      session,
      input.agents,
      input.resolveHostProfile(session.agentId),
      input.attentionItems.filter((item) => item.sessionId === session.sessionId).length,
    )),
  ].sort(compareSessionRailItems)

  const acpSessionsById = new Map(
    input.acpSessions.map((session) => [session.sessionId, session]),
  )
  const requests = [
    ...input.adapterRequests.map(projectAdapterRequest),
    ...input.attentionItems.map((item) => projectAttentionItem(
      item,
      acpSessionsById.get(item.sessionId),
    )),
  ]
    .filter((item) => matchesUnifiedWorkbenchRequestFilter(item, input.filter))
    .sort(compareRequestListItems)

  return { sessions, requests }
}

function projectAdapterSession(
  session: HostSessionSummary,
  profile: HostProfile,
): SessionRailItem {
  return {
    key: sessionRailKey('adapter', session.host_id, session.host_session_id),
    origin: 'adapter',
    hostId: session.host_id,
    sessionId: session.host_session_id,
    title: session.title,
    hostLabel: profile.label,
    hostIconSvg: profile.icon_svg,
    requestCount: session.request_count,
    pendingCount: session.pending_count,
    updatedAt: session.updated_at,
    pinnedAt: session.pinned_at,
    status: null,
    canRename: true,
    canPin: true,
    canArchive: true,
  }
}

function projectManagedAcpSession(
  session: AcpSessionSummary,
  agents: AgentSummary[],
  hostProfile: HostProfile,
  requestCount: number,
): SessionRailItem {
  const profile = resolveAgentProfile(agents, session.agentId)
  return {
    key: sessionRailKey('managed_acp', session.agentId, session.sessionId),
    origin: 'managed_acp',
    hostId: session.agentId,
    sessionId: session.sessionId,
    title: session.title,
    hostLabel: profile.label,
    // Reuse the Adapter profile artwork whenever the ACP Agent represents a
    // known host. The generic ACP icon remains the explicit fallback.
    hostIconSvg: hostProfile.icon_svg || profile.iconSvg,
    requestCount,
    pendingCount: session.pendingCount,
    updatedAt: session.updatedAt,
    pinnedAt: session.pinnedAt,
    status: session.status,
    canRename: true,
    canPin: true,
    // Restore UI for Managed ACP Sessions lands with the shared archive view.
    canArchive: false,
  }
}

function compareSessionRailItems(left: SessionRailItem, right: SessionRailItem): number {
  return compareNullableIsoDesc(left.pinnedAt, right.pinnedAt)
    || right.updatedAt.localeCompare(left.updatedAt)
    || left.key.localeCompare(right.key)
}

function compareNullableIsoDesc(
  left: string | null | undefined,
  right: string | null | undefined,
): number {
  if (left === right) return 0
  if (!left) return 1
  if (!right) return -1
  return right.localeCompare(left)
}

export function matchesUnifiedWorkbenchRequestFilter(
  item: UnifiedWorkbenchRequestListItem,
  filter: UnifiedWorkbenchFilter | undefined,
): boolean {
  if (filter?.sessionKey && item.sessionKey !== filter.sessionKey) return false
  const search = filter?.search.trim().toLowerCase() ?? ''
  if (!search) return true
  return [
    item.rawRequestId,
    item.title,
    item.summary,
    item.agentId,
    item.sourceHint ?? '',
  ].some((value) => value.toLowerCase().includes(search))
}

function compareRequestListItems(
  left: UnifiedWorkbenchRequestListItem,
  right: UnifiedWorkbenchRequestListItem,
): number {
  return right.updatedAt.localeCompare(left.updatedAt) || left.key.localeCompare(right.key)
}

export function projectAttentionItem(
  item: AttentionItem,
  session: AcpSessionSummary | undefined,
): UnifiedWorkbenchRequestListItem {
  const summary = item.kind === 'feedback'
    ? item.summary
    : item.kind === 'permission'
      ? item.description
      : item.prompt
  const agentId = session?.agentId ?? 'agent'
  const sessionKey = sessionRailKey('managed_acp', agentId, item.sessionId)
  return {
    key: workbenchRequestKey('managed_acp', sessionKey, item.id),
    origin: 'managed_acp',
    rawRequestId: item.id,
    sessionKey,
    id: item.id,
    kind: item.kind,
    title: item.title,
    summary,
    status: item.status === 'waiting'
      ? 'waiting'
      : item.status === 'cancelled'
        ? 'cancelled'
        : 'completed',
    sessionId: item.sessionId,
    agentId,
    sourceHint: session?.agentLabel ?? null,
    createdAt: item.createdAt,
    updatedAt: item.kind === 'feedback' ? (item.updatedAt ?? item.createdAt) : item.createdAt,
  }
}

export function projectAdapterRequest(
  request: FeedbackRequestSummary,
): UnifiedWorkbenchRequestListItem {
  const sessionKey = sessionRailKey(
    'adapter',
    request.host_id,
    request.host_session_id,
  )
  return {
    key: workbenchRequestKey('adapter', sessionKey, request.request_id),
    origin: 'adapter',
    rawRequestId: request.request_id,
    sessionKey,
    id: request.request_id,
    kind: 'feedback',
    title: request.title,
    summary: request.what_happened,
    status: request.status,
    sessionId: request.host_session_id,
    agentId: request.host_id,
    sourceHint: request.source_hint,
    createdAt: request.created_at,
    updatedAt: request.updated_at,
  }
}

export function resolveAgentProfile(
  agents: AgentSummary[],
  agentId: string,
): RequestListAgentProfile {
  const agent = agents.find((candidate) => candidate.id === agentId)
  return {
    id: agentId,
    label: agent?.label ?? agentId,
    iconSvg: agent?.iconSvg || (agentId === 'codex' ? TERMINAL_ICON : BOT_ICON),
  }
}

export function projectFeedbackWorkspace(
  item: FeedbackAttentionItem,
  session: AcpSessionSummary | undefined,
  persistedDraft?: DraftSnapshotV3 | null,
): FeedbackWorkspaceView {
  const documentJson = persistedDraft?.documentJson ?? (item.draftDocument === null
    ? null
    : typeof item.draftDocument === 'string'
      ? item.draftDocument
      : JSON.stringify(item.draftDocument))
  return {
    request: {
      request_id: item.id,
      host_id: session?.agentId ?? 'agent',
      host_session_id: item.sessionId,
      source_hint: session?.agentLabel ?? null,
      title: item.title,
      what_happened: item.summary || item.instructions,
      status: item.status === 'waiting'
        ? 'waiting'
        : item.status === 'cancelled'
          ? 'cancelled'
          : 'completed',
      resolution: item.status === 'submitted'
        ? 'feedback_submitted'
        : item.status === 'cancelled'
          ? 'cancelled'
          : null,
      allow_finish: false,
      final_summary: null,
      revision: persistedDraft?.revision ?? item.draftRevision,
      created_at: item.createdAt,
      updated_at: item.createdAt,
    },
    actions: item.actions.map((instruction, index) => ({
      id: `action-${index + 1}`,
      instruction,
    })),
    context_refs: [],
    request_attachments: [],
    draft: {
      document_json: documentJson,
      body_markdown: persistedDraft?.bodyMarkdown ?? item.draftMarkdown,
      saved_revision: persistedDraft?.revision ?? item.draftRevision,
      updated_at: persistedDraft?.updatedAt ?? (item.draftRevision > 0 ? item.createdAt : null),
    },
    attachments: (persistedDraft?.artifacts ?? []).map((artifact) => ({
      attachment_id: artifact.artifactId,
      file_name: artifact.fileName,
      media_type: artifact.mediaType,
      byte_size: artifact.byteSize,
      sha256: artifact.sha256,
      position: artifact.position,
    })),
    feedback: null,
  }
}

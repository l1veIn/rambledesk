import type { FeedbackRequestSummary, HostSessionSummary } from '$lib/feedback'
import { previewFixtures } from '$lib/previewFixtures'

export const ALL_ARCHIVE_REQUEST_STATUSES = ['waiting', 'in_progress', 'completed', 'cancelled'] as const

export type SelectedArchivedItem =
  | { kind: 'session'; sessionKey: string }
  | { kind: 'request'; sessionKey: string; requestId: string }

export function archivedSessionKey(session: HostSessionSummary) {
  return `${session.host_id}\u0000${session.host_session_id}`
}

export function archivedSessionFor(sessions: readonly HostSessionSummary[], key: string) {
  return sessions.find((session) => archivedSessionKey(session) === key) ?? null
}

export function archivedRequestsFor(
  requestsBySession: Record<string, FeedbackRequestSummary[]>,
  session: HostSessionSummary,
) {
  return requestsBySession[archivedSessionKey(session)] ?? []
}

export function matchesArchiveSearch(value: string | null | undefined, query: string) {
  return (value ?? '').toLowerCase().includes(query)
}

export function escapeArchiveHtml(value: string) {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')
}

/** Wraps every case-insensitive match of `query` in a `<mark>` after escaping the text. */
export function highlightArchiveMatch(value: string | null | undefined, query: string) {
  const text = value ?? ''
  const trimmed = query.trim()
  if (!trimmed) return escapeArchiveHtml(text)
  const lowerText = text.toLowerCase()
  const lowerQuery = trimmed.toLowerCase()
  let cursor = 0
  let output = ''
  while (cursor < text.length) {
    const index = lowerText.indexOf(lowerQuery, cursor)
    if (index === -1) {
      output += escapeArchiveHtml(text.slice(cursor))
      break
    }
    output += escapeArchiveHtml(text.slice(cursor, index))
    output += `<mark class="rounded-sm bg-primary/25 px-0.5 text-inherit">${escapeArchiveHtml(
      text.slice(index, index + trimmed.length),
    )}</mark>`
    cursor = index + trimmed.length
  }
  return output
}

export function requestMatchesSession(
  request: FeedbackRequestSummary,
  session: HostSessionSummary,
) {
  return request.host_id === session.host_id && request.host_session_id === session.host_session_id
}

export function previewArchivedSessions(query: string) {
  const normalized = query.trim().toLowerCase()
  return previewFixtures.archivedHostSessions.filter((session) => {
    if (!normalized) return true
    return (
      matchesArchiveSearch(session.title, normalized) ||
      matchesArchiveSearch(session.source_hint, normalized) ||
      matchesArchiveSearch(session.host_id, normalized) ||
      matchesArchiveSearch(session.host_session_id, normalized) ||
      previewFixtures.requests.some(
        (request) =>
          requestMatchesSession(request, session) &&
          (matchesArchiveSearch(request.title, normalized) ||
            matchesArchiveSearch(request.what_happened, normalized) ||
            matchesArchiveSearch(request.source_hint, normalized) ||
            matchesArchiveSearch(request.request_id, normalized)),
      )
    )
  })
}

export function previewArchivedRequests(session: HostSessionSummary, query: string) {
  const normalized = query.trim().toLowerCase()
  return previewFixtures.requests.filter(
    (request) =>
      requestMatchesSession(request, session) &&
      (!normalized ||
        matchesArchiveSearch(request.title, normalized) ||
        matchesArchiveSearch(request.what_happened, normalized) ||
        matchesArchiveSearch(request.source_hint, normalized) ||
        matchesArchiveSearch(request.request_id, normalized) ||
        matchesArchiveSearch(request.host_id, normalized) ||
        matchesArchiveSearch(request.host_session_id, normalized)),
  )
}

export function archivedSelectionExists(
  item: SelectedArchivedItem | null,
  sessions: readonly HostSessionSummary[],
  requestsBySession: Record<string, FeedbackRequestSummary[]>,
) {
  if (!item) return false
  if (!sessions.some((session) => archivedSessionKey(session) === item.sessionKey)) return false
  if (item.kind === 'session') return true
  return (requestsBySession[item.sessionKey] ?? []).some(
    (request) => request.request_id === item.requestId,
  )
}

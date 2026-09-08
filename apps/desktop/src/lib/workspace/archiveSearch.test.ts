import { describe, expect, it } from 'vitest'

import { previewFixtures } from '$lib/previewFixtures'
import {
  archivedRequestsFor,
  archivedSelectionExists,
  archivedSessionFor,
  archivedSessionKey,
  escapeArchiveHtml,
  highlightArchiveMatch,
  matchesArchiveSearch,
  previewArchivedRequests,
  previewArchivedSessions,
  requestMatchesSession,
} from './archiveSearch'

const archivedSession = previewFixtures.archivedHostSessions[0]

describe('archive search', () => {
  it('keys a session by host and host session id', () => {
    expect(archivedSessionKey(archivedSession)).toBe(
      `${archivedSession.host_id}\u0000${archivedSession.host_session_id}`,
    )
    expect(archivedSessionFor([archivedSession], archivedSessionKey(archivedSession))).toBe(
      archivedSession,
    )
    expect(archivedSessionFor([archivedSession], 'missing')).toBeNull()
  })

  it('reads requests by the session key', () => {
    const request = previewFixtures.requests[0]
    const key = archivedSessionKey(archivedSession)
    expect(archivedRequestsFor({ [key]: [request] }, archivedSession)).toEqual([request])
    expect(archivedRequestsFor({}, archivedSession)).toEqual([])
  })

  it('escapes markup and marks every case-insensitive match', () => {
    expect(escapeArchiveHtml('<b>&"')).toBe('&lt;b&gt;&amp;&quot;')
    expect(highlightArchiveMatch('Refresh <b>', 'refresh')).toBe(
      '<mark class="rounded-sm bg-primary/25 px-0.5 text-inherit">Refresh</mark> &lt;b&gt;',
    )
    expect(highlightArchiveMatch('aBaB', 'b')).toBe(
      'a<mark class="rounded-sm bg-primary/25 px-0.5 text-inherit">B</mark>a<mark class="rounded-sm bg-primary/25 px-0.5 text-inherit">B</mark>',
    )
    expect(highlightArchiveMatch(null, '')).toBe('')
  })

  it('matches a request to its session and to the search text', () => {
    const request = previewFixtures.requests[0]
    expect(requestMatchesSession(request, archivedSession)).toBe(false)
    expect(matchesArchiveSearch('Workbench', 'work')).toBe(true)
    expect(matchesArchiveSearch(null, 'work')).toBe(false)
  })

  it('filters preview sessions and requests by the query', () => {
    expect(previewArchivedSessions('')).toHaveLength(previewFixtures.archivedHostSessions.length)
    const query = previewFixtures.archivedHostSessions[0].title.slice(0, 6)
    expect(previewArchivedSessions(query).length).toBeGreaterThan(0)
    expect(previewArchivedSessions('no-such-archived-session')).toEqual([])

    const session = previewFixtures.archivedHostSessions[0]
    expect(previewArchivedRequests(session, '')).toEqual(
      previewFixtures.requests.filter((request) => requestMatchesSession(request, session)),
    )
    expect(previewArchivedRequests(session, 'no-such-request')).toEqual([])
  })

  it('keeps a selection only while its session and request still exist', () => {
    const session = previewFixtures.archivedHostSessions[0]
    const key = archivedSessionKey(session)
    expect(archivedSelectionExists(null, [session], {})).toBe(false)
    expect(archivedSelectionExists({ kind: 'session', sessionKey: key }, [session], {})).toBe(true)
    expect(archivedSelectionExists({ kind: 'session', sessionKey: 'gone' }, [session], {})).toBe(false)
    expect(
      archivedSelectionExists({ kind: 'request', sessionKey: key, requestId: 'r1' }, [session], {
        [key]: [{ request_id: 'r1' } as never],
      }),
    ).toBe(true)
    expect(
      archivedSelectionExists({ kind: 'request', sessionKey: key, requestId: 'r2' }, [session], {
        [key]: [{ request_id: 'r1' } as never],
      }),
    ).toBe(false)
  })
})

import { describe, expect, it } from 'vitest'
import type { FeedbackRequestSummary } from '../feedback'
import { filterRequestPage } from './requestFilters'

const now = Date.parse('2026-09-03T12:00:00Z')
const day = 24 * 60 * 60 * 1000

function request(requestId: string, age: number): FeedbackRequestSummary {
  return {
    request_id: requestId, host_id: 'codex', host_session_id: 'session-1',
    title: requestId, what_happened: '', source_hint: null, status: 'waiting',
    resolution: null, allow_finish: false, final_summary: null, revision: 1,
    created_at: new Date(now - age).toISOString(), updated_at: new Date(now - age).toISOString(),
  }
}

describe('request time filters', () => {
  it.each([['24h', 1], ['7d', 7], ['30d', 30]] as const)(
    '%s includes the boundary and stops pagination once older requests begin',
    (range, days) => {
      const recent = request('recent', 1000)
      const boundary = request('boundary', days * day)
      const older = request('older', days * day + 1)
      expect(filterRequestPage({ requests: [recent, boundary, older], next_cursor: 'older-page' }, range, now))
        .toEqual({ requests: [recent, boundary], next_cursor: null })
    },
  )

  it('keeps the cursor when further pages may contain matches', () => {
    const requests = Array.from({ length: 100 }, (_, index) => request(`request-${index}`, index * 1000))
    expect(filterRequestPage({ requests, next_cursor: 'next-page' }, '24h', now))
      .toEqual({ requests, next_cursor: 'next-page' })
  })

  it('returns no more pages when every request is outside the range, and restores them for all time', () => {
    const page = { requests: [request('old-request', 60 * day)], next_cursor: 'next-page' }
    expect(filterRequestPage(page, '30d', now)).toEqual({ requests: [], next_cursor: null })
    expect(filterRequestPage(page, 'all', now)).toEqual(page)
  })
})

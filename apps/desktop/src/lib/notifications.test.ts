import { describe, expect, it } from 'vitest'

import type { FeedbackRequestSummary } from './feedback'
import {
  collectNewRequests,
  InboxNotificationTracker,
  notificationLabel,
  notificationStateForPermission,
} from './notifications'

function request(requestId: string): FeedbackRequestSummary {
  return {
    request_id: requestId,
    host_id: 'test-host',
    host_session_id: 'session',
    source_hint: 'Private source',
    title: 'Test Ramble',
    what_happened: 'Sensitive summary',
    status: 'waiting',
    revision: 0,
    created_at: '2026-07-29T00:00:00Z',
    updated_at: '2026-07-29T00:00:00Z',
  }
}

describe('notification inbox tracking', () => {
  it('marks the initial inbox known and reports only later arrivals', () => {
    const known = new Set<string>()
    expect(collectNewRequests(known, [request('one')]).map((item) => item.request_id)).toEqual([
      'one',
    ])
    expect(
      collectNewRequests(known, [request('one'), request('two')]).map((item) => item.request_id),
    ).toEqual(['two'])
    expect(collectNewRequests(known, [request('two')])).toEqual([])
  })

  it('treats whichever concurrent startup snapshot arrives first as the baseline', () => {
    const tracker = new InboxNotificationTracker()
    expect(tracker.observe([request('existing')])).toEqual([])
    expect(
      tracker
        .observe([request('existing'), request('arrived-during-startup')])
        .map((item) => item.request_id),
    ).toEqual(['arrived-during-startup'])
    expect(tracker.observe([request('arrived-during-startup')])).toEqual([])
  })

  it('keeps permission labels understandable', () => {
    expect(notificationLabel('enabled')).toBe('通知已开启')
    expect(notificationLabel('muted')).toBe('通知已暂停，点击重新开启')
    expect(notificationLabel('muted', 'en')).toBe('Notifications paused — click to enable')
    expect(notificationLabel('disabled')).toBe('启用通知')
    expect(notificationLabel('unavailable')).toBe('通知不可用')
  })

  it('keeps an OS-granted notification permission reactivatable after muting', () => {
    expect(notificationStateForPermission(true, true)).toBe('enabled')
    expect(notificationStateForPermission(true, false)).toBe('muted')
    expect(notificationStateForPermission(false, true)).toBe('disabled')
  })
})

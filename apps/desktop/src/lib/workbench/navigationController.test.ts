import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => {
  const storage = new Map<string, string>()
  Object.defineProperty(globalThis, 'localStorage', {
    configurable: true,
    value: {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
      removeItem: (key: string) => storage.delete(key),
      clear: () => storage.clear(),
    },
  })
  Object.defineProperty(globalThis, 'navigator', {
    configurable: true,
    value: { language: 'en-US' },
  })
  return {
    invoke: vi.fn(),
    sendNotification: vi.fn(),
    storage,
  }
})

vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('@tauri-apps/plugin-notification', () => ({ sendNotification: mocks.sendNotification }))

import type { FeedbackRequestSummary, HostSessionSummary, ListFeedbackRequestsOutput } from '../feedback'
import { createNavigationController, type NavigationState } from './navigationController'

function feedbackRequest(requestId: string): FeedbackRequestSummary {
  return {
    request_id: requestId,
    host_id: 'codex',
    host_session_id: 'session-1',
    source_hint: 'Workbench',
    title: 'Review refresh behavior',
    what_happened: 'The refresh button should update page data.',
    status: 'waiting',
    resolution: null,
    allow_finish: false,
    final_summary: null,
    revision: 1,
    created_at: '2026-08-22T00:00:00Z',
    updated_at: '2026-08-22T00:01:00Z',
  }
}

function hostSession(): HostSessionSummary {
  return {
    host_id: 'codex',
    host_session_id: 'session-1',
    title: 'Refresh workbench',
    source_hint: 'Workbench',
    request_count: 1,
    pending_count: 1,
    updated_at: '2026-08-22T00:01:00Z',
    pinned_at: null,
    archived_at: null,
    host_pinned_at: null,
  }
}

function createController() {
  return createNavigationController({
    isTauri: true,
    previewMode: false,
    tr: (source) => source,
    messageFrom: (cause) => String(cause),
    getNotificationState: () => 'disabled',
    getWorkspaceRequestId: () => undefined,
    isDirty: () => false,
    saveDraftNow: vi.fn(async () => true),
    openRequest: vi.fn(async () => undefined),
    clearWorkspace: vi.fn(),
    onPageError: vi.fn(),
    canSendOsBanners: () => false,
  })
}

describe('navigationController', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    mocks.invoke.mockReset()
    mocks.sendNotification.mockReset()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('refreshes page navigation and requests with a minimum loading duration', async () => {
    const firstList: ListFeedbackRequestsOutput = { requests: [], next_cursor: null }
    const refreshedRequest = feedbackRequest('request-1')
    const refreshedList: ListFeedbackRequestsOutput = {
      requests: [refreshedRequest],
      next_cursor: 'next-page',
    }
    let requestListOutput = firstList
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'list_feedback_inbox') return [refreshedRequest]
      if (command === 'list_host_sessions') return [hostSession()]
      if (command === 'list_host_profiles') return []
      if (command === 'list_feedback_requests') return requestListOutput
      if (command === 'set_pending_count') return undefined
      return undefined
    })

    const controller = createController()
    let state: NavigationState | undefined
    const unsubscribe = controller.subscribe((next) => {
      state = next
    })

    try {
      await controller.initialize()
      expect(state?.loadingNavigation).toBe(false)
      expect(state?.loadingRequests).toBe(false)
      expect(state?.refreshingPage).toBe(false)

      requestListOutput = refreshedList
      const refresh = controller.refreshPage(300)
      await Promise.resolve()
      await Promise.resolve()

      expect(state?.loadingNavigation).toBe(true)
      expect(state?.loadingRequests).toBe(true)
      expect(state?.refreshingPage).toBe(true)
      expect(mocks.invoke).toHaveBeenCalledWith('list_feedback_inbox')
      expect(mocks.invoke).toHaveBeenCalledWith('list_host_sessions')
      expect(mocks.invoke).toHaveBeenCalledWith('list_feedback_requests', {
        input: {
          host_id: null,
          host_session_id: null,
          status: ['waiting', 'in_progress', 'completed', 'cancelled'],
          archived: null,
          search: null,
          limit: 100,
          cursor: null,
        },
      })

      await vi.advanceTimersByTimeAsync(299)
      expect(state?.loadingNavigation).toBe(true)
      expect(state?.loadingRequests).toBe(true)
      expect(state?.refreshingPage).toBe(true)

      await vi.advanceTimersByTimeAsync(1)
      await refresh

      expect(state?.loadingNavigation).toBe(false)
      expect(state?.loadingRequests).toBe(false)
      expect(state?.refreshingPage).toBe(false)
      expect(state?.requests).toEqual([refreshedRequest])
      expect(state?.nextRequestCursor).toBe('next-page')
      expect(state?.hostSessions).toEqual([hostSession()])
    } finally {
      unsubscribe()
    }
  })
})

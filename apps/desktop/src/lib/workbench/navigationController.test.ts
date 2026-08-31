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

function createController(
  overrides: Partial<Parameters<typeof createNavigationController>[0]> = {},
) {
  return createNavigationController({
    isTauri: true,
    previewMode: false,
    tr: (source) => source,
    messageFrom: (cause) => String(cause),
    getNotificationState: () => 'disabled',
    getWorkspaceRequestId: () => undefined,
    isDirty: () => false,
    saveDraftNow: vi.fn(async () => true),
    openRequest: vi.fn(async () => true),
    clearWorkspace: vi.fn(),
    onPageError: vi.fn(),
    canSendOsBanners: () => false,
    ...overrides,
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

  it('loads navigation facts without opening a default request when workspace restore is pending', async () => {
    const request = feedbackRequest('request-restore')
    const openRequest = vi.fn(async () => true)
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'list_feedback_inbox') return [request]
      if (command === 'list_host_sessions') return [hostSession()]
      if (command === 'list_host_profiles') return []
      if (command === 'list_feedback_requests') {
        return { requests: [request], next_cursor: null } satisfies ListFeedbackRequestsOutput
      }
      return undefined
    })
    const controller = createController({ openRequest })

    await controller.initialize(false)

    expect(openRequest).not.toHaveBeenCalled()
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

  it('returns to the visible host scope after archiving the selected flat-rail session', async () => {
    const selectedSession = { ...hostSession(), pending_count: 0 }
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'list_feedback_inbox') return []
      if (command === 'list_host_sessions') return [selectedSession]
      if (command === 'list_host_profiles') return []
      if (command === 'list_feedback_requests') {
        return { requests: [], next_cursor: null } satisfies ListFeedbackRequestsOutput
      }
      if (command === 'archive_host_session') return { ...selectedSession, archived_at: '2026-09-01T00:00:00Z' }
      return undefined
    })

    const controller = createController()
    let state: NavigationState | undefined
    const unsubscribe = controller.subscribe((next) => {
      state = next
    })

    try {
      await controller.initialize()
      await controller.selectScope(selectedSession.host_id, selectedSession.host_session_id)
      expect(state?.selectedHostId).toBe('codex')
      expect(state?.selectedHostSessionId).toBe('session-1')

      await controller.archiveHostSession(selectedSession)

      expect(state?.selectedHostId).toBe('codex')
      expect(state?.selectedHostSessionId).toBeNull()
      expect(mocks.invoke).toHaveBeenLastCalledWith('list_feedback_requests', {
        input: {
          host_id: 'codex',
          host_session_id: null,
          status: ['waiting', 'in_progress', 'completed', 'cancelled'],
          archived: null,
          search: null,
          limit: 100,
          cursor: null,
        },
      })
    } finally {
      unsubscribe()
    }
  })

  it('returns the request snapshot for the selected session scope', async () => {
    const selectedRequest = feedbackRequest('selected-request')
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'list_feedback_requests') {
        return { requests: [selectedRequest], next_cursor: null } satisfies ListFeedbackRequestsOutput
      }
      return []
    })

    const controller = createController()
    const result = await controller.selectScope('codex', 'session-1')

    expect(result).toEqual({ selected: true, requests: [selectedRequest] })
    expect(mocks.invoke).toHaveBeenCalledWith('list_feedback_requests', {
      input: expect.objectContaining({ host_id: 'codex', host_session_id: 'session-1' }),
    })
  })

  it('does not select a new scope when saving the current draft fails', async () => {
    const controller = createController({
      isDirty: () => true,
      saveDraftNow: vi.fn(async () => false),
    })
    let state: NavigationState | undefined
    const unsubscribe = controller.subscribe((next) => (state = next))

    try {
      const result = await controller.selectScope('codex', 'session-1')

      expect(result.selected).toBe(false)
      expect(state?.selectedHostId).toBeNull()
      expect(mocks.invoke).not.toHaveBeenCalledWith('list_feedback_requests', expect.anything())
    } finally {
      unsubscribe()
    }
  })

  it('ignores a stale request-list response after a newer scope wins', async () => {
    let resolveFirst: ((value: ListFeedbackRequestsOutput) => void) | undefined
    const firstResult = new Promise<ListFeedbackRequestsOutput>((resolve) => {
      resolveFirst = resolve
    })
    const secondRequest = { ...feedbackRequest('second-request'), host_session_id: 'session-2' }
    mocks.invoke.mockImplementation(
      async (command: string, input?: { input?: { host_session_id?: string } }) => {
        if (command !== 'list_feedback_requests') return []
        if (input?.input?.host_session_id === 'session-1') return firstResult
        return { requests: [secondRequest], next_cursor: null } satisfies ListFeedbackRequestsOutput
      },
    )

    const controller = createController()
    let state: NavigationState | undefined
    const unsubscribe = controller.subscribe((next) => (state = next))

    try {
      const first = controller.selectScope('codex', 'session-1')
      await Promise.resolve()
      const second = await controller.selectScope('codex', 'session-2')
      resolveFirst?.({ requests: [feedbackRequest('stale-request')], next_cursor: null })
      const stale = await first

      expect(second.selected).toBe(true)
      expect(stale.selected).toBe(false)
      expect(state?.selectedHostSessionId).toBe('session-2')
      expect(state?.requests).toEqual([secondRequest])
      expect(state?.loadingRequests).toBe(false)
    } finally {
      unsubscribe()
    }
  })

  it('rolls back the prior scope and request snapshot when the target refresh fails', async () => {
    const priorRequest = feedbackRequest('prior-request')
    let rejectTarget = false
    mocks.invoke.mockImplementation(
      async (command: string, input?: { input?: { host_session_id?: string } }) => {
        if (command !== 'list_feedback_requests') return []
        if (rejectTarget && input?.input?.host_session_id === 'session-2') {
          throw new Error('target refresh failed')
        }
        return {
          requests: [priorRequest],
          next_cursor: 'prior-cursor',
        } satisfies ListFeedbackRequestsOutput
      },
    )

    const controller = createController()
    let state: NavigationState | undefined
    const unsubscribe = controller.subscribe((next) => (state = next))

    try {
      await controller.selectScope('codex', 'session-1')
      rejectTarget = true

      const result = await controller.selectScope('codex', 'session-2')

      expect(result.selected).toBe(false)
      expect(state?.selectedHostId).toBe('codex')
      expect(state?.selectedHostSessionId).toBe('session-1')
      expect(state?.requests).toEqual([priorRequest])
      expect(state?.nextRequestCursor).toBe('prior-cursor')
    } finally {
      unsubscribe()
    }
  })

  it('does not roll back a newer scope when an older target refresh fails late', async () => {
    let rejectOlder: ((cause: Error) => void) | undefined
    const olderRefresh = new Promise<ListFeedbackRequestsOutput>((_, reject) => {
      rejectOlder = reject
    })
    const newestRequest = { ...feedbackRequest('newest-request'), host_session_id: 'session-3' }
    mocks.invoke.mockImplementation(
      async (command: string, input?: { input?: { host_session_id?: string } }) => {
        if (command !== 'list_feedback_requests') return []
        if (input?.input?.host_session_id === 'session-2') return olderRefresh
        return {
          requests: [newestRequest],
          next_cursor: 'newest-cursor',
        } satisfies ListFeedbackRequestsOutput
      },
    )

    const onPageError = vi.fn()
    const controller = createController({ onPageError })
    let state: NavigationState | undefined
    const unsubscribe = controller.subscribe((next) => (state = next))

    try {
      const older = controller.selectScope('codex', 'session-2')
      await Promise.resolve()
      const newest = await controller.selectScope('codex', 'session-3')
      rejectOlder?.(new Error('older refresh failed'))
      const stale = await older

      expect(newest.selected).toBe(true)
      expect(stale.selected).toBe(false)
      expect(state?.selectedHostSessionId).toBe('session-3')
      expect(state?.requests).toEqual([newestRequest])
      expect(state?.nextRequestCursor).toBe('newest-cursor')
      expect(onPageError).not.toHaveBeenCalled()
    } finally {
      unsubscribe()
    }
  })
})

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { get } from 'svelte/store'

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
    applicationCall: vi.fn(),
    notificationSend: vi.fn(),
    setPendingCount: vi.fn(),
    storage,
  }
})

import type { FeedbackRequestSummary, HostSessionSummary, ListFeedbackRequestsOutput } from '../feedback'
import { TestApplicationTransport } from '../application/testApplicationTransport'
import { createUnavailableWorkbenchCapabilities } from '../capabilities/unavailableCapabilities'
import { createNavigationController, type NavigationState } from './navigationController'

const unavailableCapabilities = createUnavailableWorkbenchCapabilities()

function testCapabilities() {
  return {
    notifications: {
      status: { availability: 'available', source: 'native' } as const,
      implementation: {
        ...unavailableCapabilities.notifications.implementation,
        send: mocks.notificationSend,
      },
    },
    tray: {
      status: { availability: 'available', source: 'native' } as const,
      implementation: { setPendingCount: mocks.setPendingCount },
    },
  }
}

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

function hostSession(overrides: Partial<HostSessionSummary> = {}): HostSessionSummary {
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
    ...overrides,
  }
}

function createController(
  overrides: Partial<Parameters<typeof createNavigationController>[0]> = {},
) {
  const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
    .handle('listFeedbackInbox', (input) => mocks.applicationCall('listFeedbackInbox', input))
    .handle('listHostSessions', (input) => mocks.applicationCall('listHostSessions', input))
    .handle('listHostProfiles', (input) => mocks.applicationCall('listHostProfiles', input))
    .handle('listFeedbackRequests', (input) => mocks.applicationCall('listFeedbackRequests', input))
    .handle('renameHostSession', (input) => mocks.applicationCall('renameHostSession', input))
    .handle('setHostSessionPinned', (input) => mocks.applicationCall('setHostSessionPinned', input))
    .handle('archiveHostSession', (input) => mocks.applicationCall('archiveHostSession', input))
    .handle('setHostPinned', (input) => mocks.applicationCall('setHostPinned', input))
  return createNavigationController({
    capabilities: testCapabilities(),
    previewMode: false,
    transport,
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
    mocks.applicationCall.mockReset()
    mocks.notificationSend.mockReset()
    mocks.notificationSend.mockResolvedValue(undefined)
    mocks.setPendingCount.mockReset()
    mocks.setPendingCount.mockResolvedValue(undefined)
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('combines status filters with the selected session and search without changing the open request', async () => {
    const openRequest = vi.fn(async () => true)
    const clearWorkspace = vi.fn()
    mocks.applicationCall.mockResolvedValue({ requests: [feedbackRequest('match')], next_cursor: null })
    const controller = createController({ openRequest, clearWorkspace })
    await controller.selectScope('codex', 'session-1')
    await controller.setRequestSearch('refresh')
    await controller.setRequestFilters({ status: 'pending', timeRange: 'all' })

    expect(mocks.applicationCall).toHaveBeenLastCalledWith('listFeedbackRequests', {
      host_id: 'codex', host_session_id: 'session-1', search: 'refresh',
      status: ['waiting', 'in_progress'], archived: null, limit: 100, cursor: null,
    })
    expect(openRequest).not.toHaveBeenCalled()
    expect(clearWorkspace).not.toHaveBeenCalled()

    await controller.setRequestFilters({ status: 'all', timeRange: 'all' })
    expect(mocks.applicationCall).toHaveBeenLastCalledWith('listFeedbackRequests', expect.objectContaining({
      status: ['waiting', 'in_progress', 'completed', 'cancelled'], search: 'refresh',
    }))
  })

  it('discards an old load-more response after the filters change', async () => {
    let releasePage!: (page: ListFeedbackRequestsOutput) => void
    const oldPage = new Promise<ListFeedbackRequestsOutput>((resolve) => { releasePage = resolve })
    const completed = { ...feedbackRequest('completed'), status: 'completed' as const }
    mocks.applicationCall.mockImplementation(async (_command, input) => {
      if (input.cursor) return oldPage
      if (input.status.length === 1) return { requests: [completed], next_cursor: null }
      return { requests: [feedbackRequest('waiting')], next_cursor: 'page-2' }
    })
    const controller = createController()
    await controller.refreshRequests()
    const loadingMore = controller.loadMoreRequests()
    await controller.setRequestFilters({ status: 'completed', timeRange: 'all' })
    releasePage({ requests: [feedbackRequest('stale')], next_cursor: 'stale-cursor' })
    await loadingMore

    expect(get(controller)).toMatchObject({
      requests: [completed], nextRequestCursor: null, loadingMoreRequests: false,
      requestFilters: { status: 'completed', timeRange: 'all' },
    })
  })

  it('does not let a page refresh replace a newer filtered list', async () => {
    let releaseRefresh!: (page: ListFeedbackRequestsOutput) => void
    const oldPage = new Promise<ListFeedbackRequestsOutput>((resolve) => { releaseRefresh = resolve })
    const completed = { ...feedbackRequest('completed'), status: 'completed' as const }
    mocks.applicationCall.mockImplementation(async (command, input) => {
      if (command !== 'listFeedbackRequests') return []
      return input.status.length === 1 ? { requests: [completed], next_cursor: null } : oldPage
    })
    const controller = createController()
    const refreshing = controller.refreshPage(0)
    await controller.setRequestFilters({ status: 'completed', timeRange: 'all' })
    releaseRefresh({ requests: [feedbackRequest('stale')], next_cursor: 'stale-cursor' })
    await refreshing

    expect(get(controller)).toMatchObject({ requests: [completed], nextRequestCursor: null, loadingRequests: false })
  })

  it('applies status and time filters consistently in preview mode', async () => {
    vi.setSystemTime(new Date('2026-09-03T12:00:00Z'))
    const controller = createController({ previewMode: true })
    await controller.initialize(false)
    await controller.setRequestFilters({ status: 'pending', timeRange: 'all' })
    expect(get(controller).requests.map((request) => request.status).sort()).toEqual(['in_progress', 'waiting'])
    await controller.setRequestFilters({ status: 'pending', timeRange: '24h' })
    expect(get(controller).requests).toEqual([])
    await controller.setRequestFilters({ status: 'all', timeRange: 'all' })
    expect(get(controller).requests).toHaveLength(4)
  })

  it('loads navigation facts without opening a default request when workspace restore is pending', async () => {
    const request = feedbackRequest('request-restore')
    const openRequest = vi.fn(async () => true)
    mocks.applicationCall.mockImplementation(async (command: string) => {
      if (command === 'listFeedbackInbox') return [request]
      if (command === 'listHostSessions') return [hostSession()]
      if (command === 'listHostProfiles') return []
      if (command === 'listFeedbackRequests') {
        return { requests: [request], next_cursor: null } satisfies ListFeedbackRequestsOutput
      }
      return undefined
    })
    const controller = createController({ openRequest })

    await expect(controller.initialize(false)).resolves.toBe(true)

    expect(openRequest).not.toHaveBeenCalled()
    expect(mocks.setPendingCount).toHaveBeenCalledWith(1)
  })

  it('waits for transport readiness before loading application facts', async () => {
    const transport = new TestApplicationTransport(undefined)
      .handle('listFeedbackInbox', (input) => mocks.applicationCall('listFeedbackInbox', input))
      .handle('listHostSessions', (input) => mocks.applicationCall('listHostSessions', input))
      .handle('listHostProfiles', (input) => mocks.applicationCall('listHostProfiles', input))
      .handle('listFeedbackRequests', (input) => mocks.applicationCall('listFeedbackRequests', input))
    mocks.applicationCall.mockImplementation(async (command: string) => {
      if (command === 'listHostSessions') return [hostSession()]
      if (command === 'listFeedbackRequests') {
        return { requests: [], next_cursor: null } satisfies ListFeedbackRequestsOutput
      }
      return []
    })
    const controller = createController({ transport })
    const initializing = controller.initialize(false)

    await Promise.resolve()
    expect(mocks.applicationCall).not.toHaveBeenCalled()
    transport.markReady()
    await expect(initializing).resolves.toBe(true)
    expect(mocks.applicationCall).toHaveBeenCalledWith('listHostSessions', undefined)
  })

  it('preserves existing navigation facts when a later readiness check fails', async () => {
    class ToggleReadyTransport extends TestApplicationTransport {
      failReadiness = false

      override waitUntilReady(): Promise<void> {
        return this.failReadiness
          ? Promise.reject(new Error('authenticated session expired'))
          : super.waitUntilReady()
      }
    }
    const transport = new ToggleReadyTransport(undefined, { initiallyReady: true })
      .handle('listFeedbackInbox', (input) => mocks.applicationCall('listFeedbackInbox', input))
      .handle('listHostSessions', (input) => mocks.applicationCall('listHostSessions', input))
      .handle('listHostProfiles', (input) => mocks.applicationCall('listHostProfiles', input))
      .handle('listFeedbackRequests', (input) => mocks.applicationCall('listFeedbackRequests', input))
    mocks.applicationCall.mockImplementation(async (command: string) => {
      if (command === 'listHostSessions') return [hostSession()]
      if (command === 'listFeedbackRequests') {
        return { requests: [], next_cursor: null } satisfies ListFeedbackRequestsOutput
      }
      return []
    })
    const onPageError = vi.fn()
    const controller = createController({ transport, onPageError })
    let state: NavigationState | undefined
    const unsubscribe = controller.subscribe((next) => (state = next))

    try {
      await expect(controller.initialize(false)).resolves.toBe(true)
      transport.failReadiness = true
      await expect(controller.initialize(false)).resolves.toBe(false)

      expect(state?.hostSessions).toEqual([hostSession()])
      expect(state?.hostSessionFactsStatus).toBe('failed')
      expect(onPageError).toHaveBeenCalledWith('Error: authenticated session expired')
    } finally {
      unsubscribe()
    }
  })

  it('reports when initial navigation facts could not be loaded', async () => {
    mocks.applicationCall.mockRejectedValueOnce(new Error('navigation unavailable'))
    const onPageError = vi.fn()
    const controller = createController({ onPageError })

    await expect(controller.initialize(false)).resolves.toBe(false)

    expect(onPageError).toHaveBeenCalledWith('Error: navigation unavailable')
  })

  it('keeps the newest host-session facts when an older refresh finishes late', async () => {
    let releaseOlder: ((sessions: HostSessionSummary[]) => void) | undefined
    const olderSessions = new Promise<HostSessionSummary[]>((resolve) => (releaseOlder = resolve))
    let hostSessionCalls = 0
    mocks.applicationCall.mockImplementation(async (command: string) => {
      if (command === 'listFeedbackInbox') return []
      if (command === 'listHostSessions') {
        hostSessionCalls += 1
        return hostSessionCalls === 1
          ? olderSessions
          : [hostSession({ host_session_id: 'newest-session' })]
      }
      return undefined
    })
    const controller = createController()
    let state: NavigationState | undefined
    const unsubscribe = controller.subscribe((next) => (state = next))

    try {
      const older = controller.refreshNavigation()
      await Promise.resolve()
      const newest = controller.refreshNavigation()
      await expect(newest).resolves.toBe(true)
      releaseOlder?.([hostSession({ host_session_id: 'older-session' })])
      await expect(older).resolves.toBe(true)

      expect(state?.hostSessions[0]?.host_session_id).toBe('newest-session')
      expect(state?.hostSessionFactsStatus).toBe('ready')
      expect(state?.hostSessionFactsRevision).toBe(1)
    } finally {
      unsubscribe()
    }
  })

  it('marks failed facts without discarding the last known host sessions', async () => {
    const previous = hostSession()
    let failing = false
    mocks.applicationCall.mockImplementation(async (command: string) => {
      if (command === 'listFeedbackInbox') return []
      if (command === 'listHostSessions') {
        if (failing) throw new Error('host sessions unavailable')
        return [previous]
      }
      if (command === 'listHostProfiles') return []
      if (command === 'listFeedbackRequests') {
        return { requests: [], next_cursor: null } satisfies ListFeedbackRequestsOutput
      }
      return undefined
    })
    const controller = createController()
    let state: NavigationState | undefined
    const unsubscribe = controller.subscribe((next) => (state = next))

    try {
      await controller.initialize(false)
      const readyRevision = state?.hostSessionFactsRevision
      failing = true
      await expect(controller.refreshNavigation()).resolves.toBe(false)

      expect(state?.hostSessions).toEqual([previous])
      expect(state?.hostSessionFactsStatus).toBe('failed')
      expect(state?.hostSessionFactsRevision).toBe((readyRevision ?? 0) + 1)
    } finally {
      unsubscribe()
    }
  })

  it('advances the facts revision when an identical refresh can retry recovery', async () => {
    mocks.applicationCall.mockImplementation(async (command: string) => {
      if (command === 'listFeedbackInbox') return []
      if (command === 'listHostSessions') return [hostSession()]
      if (command === 'listHostProfiles') return []
      if (command === 'listFeedbackRequests') {
        return { requests: [], next_cursor: null } satisfies ListFeedbackRequestsOutput
      }
      return undefined
    })
    const controller = createController()
    let state: NavigationState | undefined
    const unsubscribe = controller.subscribe((next) => (state = next))

    try {
      await controller.initialize(false)
      const initialRevision = state?.hostSessionFactsRevision
      await controller.refreshNavigation()

      expect(state?.hostSessionFactsRevision).toBe((initialRevision ?? 0) + 1)
    } finally {
      unsubscribe()
    }
  })

  it('refreshes page navigation and requests with a minimum loading duration', async () => {
    const firstList: ListFeedbackRequestsOutput = { requests: [], next_cursor: null }
    const refreshedRequest = feedbackRequest('request-1')
    const refreshedList: ListFeedbackRequestsOutput = {
      requests: [refreshedRequest],
      next_cursor: 'next-page',
    }
    let requestListOutput = firstList
    mocks.applicationCall.mockImplementation(async (command: string) => {
      if (command === 'listFeedbackInbox') return [refreshedRequest]
      if (command === 'listHostSessions') return [hostSession()]
      if (command === 'listHostProfiles') return []
      if (command === 'listFeedbackRequests') return requestListOutput
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
      expect(mocks.applicationCall).toHaveBeenCalledWith('listFeedbackInbox', undefined)
      expect(mocks.applicationCall).toHaveBeenCalledWith('listHostSessions', undefined)
      expect(mocks.applicationCall).toHaveBeenCalledWith('listFeedbackRequests', {
          host_id: null,
          host_session_id: null,
          status: ['waiting', 'in_progress', 'completed', 'cancelled'],
          archived: null,
          search: null,
          limit: 100,
          cursor: null,
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
    mocks.applicationCall.mockImplementation(async (command: string) => {
      if (command === 'listFeedbackInbox') return []
      if (command === 'listHostSessions') return [selectedSession]
      if (command === 'listHostProfiles') return []
      if (command === 'listFeedbackRequests') {
        return { requests: [], next_cursor: null } satisfies ListFeedbackRequestsOutput
      }
      if (command === 'archiveHostSession') return { ...selectedSession, archived_at: '2026-09-01T00:00:00Z' }
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
      expect(mocks.applicationCall).toHaveBeenCalledWith('listFeedbackRequests', {
          host_id: 'codex',
          host_session_id: null,
          status: ['waiting', 'in_progress', 'completed', 'cancelled'],
          archived: null,
          search: null,
          limit: 100,
          cursor: null,
      })
    } finally {
      unsubscribe()
    }
  })

  it('returns the request snapshot for the selected session scope', async () => {
    const selectedRequest = feedbackRequest('selected-request')
    mocks.applicationCall.mockImplementation(async (command: string) => {
      if (command === 'listFeedbackRequests') {
        return { requests: [selectedRequest], next_cursor: null } satisfies ListFeedbackRequestsOutput
      }
      return []
    })

    const controller = createController()
    const result = await controller.selectScope('codex', 'session-1')

    expect(result).toEqual({ selected: true, requests: [selectedRequest] })
    expect(mocks.applicationCall).toHaveBeenCalledWith(
      'listFeedbackRequests',
      expect.objectContaining({ host_id: 'codex', host_session_id: 'session-1' }),
    )
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
      expect(mocks.applicationCall).not.toHaveBeenCalledWith('listFeedbackRequests', expect.anything())
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
    mocks.applicationCall.mockImplementation(
      async (command: string, input?: { host_session_id?: string }) => {
        if (command !== 'listFeedbackRequests') return []
        if (input?.host_session_id === 'session-1') return firstResult
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
    mocks.applicationCall.mockImplementation(
      async (command: string, input?: { host_session_id?: string }) => {
        if (command !== 'listFeedbackRequests') return []
        if (rejectTarget && input?.host_session_id === 'session-2') {
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
    mocks.applicationCall.mockImplementation(
      async (command: string, input?: { host_session_id?: string }) => {
        if (command !== 'listFeedbackRequests') return []
        if (input?.host_session_id === 'session-2') return olderRefresh
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

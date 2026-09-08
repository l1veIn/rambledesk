import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { get } from 'svelte/store'

import { APPLICATION_READ_TIMEOUT_MS } from '../application/applicationReadTimeout'
import { TestApplicationTransport } from '../application/testApplicationTransport'
import type { HostSessionSummary, ListFeedbackRequestsOutput } from '../feedback'
import type { NavigationState } from './navigationController'
import {
  createController,
  feedbackRequest,
  hostSession,
  mocks,
  resetNavigationTestMocks,
} from './navigationControllerTestHarness'

describe('navigationController request list', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    resetNavigationTestMocks()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('keeps the displayed list interactive during a background refresh and updates it in place', async () => {
    const request = feedbackRequest('visible-request')
    const updated = { ...request, status: 'completed' as const, revision: 2 }
    const openRequest = vi.fn(async () => true)
    const clearWorkspace = vi.fn()
    mocks.applicationCall.mockResolvedValueOnce({ requests: [request], next_cursor: null })
    const controller = createController({
      openRequest, clearWorkspace, getWorkspaceRequestId: () => request.request_id,
    })
    await controller.refreshRequests()
    const displayedRows = get(controller).requests
    let releaseRefresh!: (page: ListFeedbackRequestsOutput) => void
    mocks.applicationCall.mockReturnValueOnce(new Promise<ListFeedbackRequestsOutput>((resolve) => {
      releaseRefresh = resolve
    }))
    const loadingStates: boolean[] = []
    const unsubscribe = controller.subscribe((state) => loadingStates.push(state.loadingRequests))
    try {
      const refreshing = controller.refreshRequests()
      expect(get(controller).requests).toBe(displayedRows)
      expect(get(controller).loadingRequests).toBe(false)

      releaseRefresh({ requests: [updated], next_cursor: null })
      await refreshing

      expect(loadingStates.every((loading) => !loading)).toBe(true)
      expect(get(controller).requests).toEqual([updated])
      expect(openRequest).not.toHaveBeenCalled()
      expect(clearWorkspace).not.toHaveBeenCalled()
    } finally {
      unsubscribe()
    }
  })

  it('shows the first load but keeps a previously loaded empty state stable on refresh failure', async () => {
    let releaseInitial!: (page: ListFeedbackRequestsOutput) => void
    mocks.applicationCall.mockReturnValueOnce(new Promise<ListFeedbackRequestsOutput>((resolve) => {
      releaseInitial = resolve
    }))
    const onPageError = vi.fn()
    const controller = createController({ onPageError })
    const initializing = controller.refreshRequests()
    expect(get(controller).loadingRequests).toBe(true)
    releaseInitial({ requests: [], next_cursor: null })
    await initializing

    mocks.applicationCall.mockRejectedValueOnce(new Error('refresh unavailable'))
    const refreshing = controller.refreshRequests()
    expect(get(controller).loadingRequests).toBe(false)
    await refreshing

    expect(get(controller)).toMatchObject({ loadingRequests: false, requests: [] })
    expect(onPageError).toHaveBeenCalledWith('Error: refresh unavailable')
  })

  it.each(['scope', 'search', 'filter'] as const)('still shows loading when the %s changes', async (change) => {
    mocks.applicationCall.mockResolvedValueOnce({ requests: [feedbackRequest('original')], next_cursor: null })
    const controller = createController()
    await controller.refreshRequests()
    let releaseQuery!: (page: ListFeedbackRequestsOutput) => void
    mocks.applicationCall.mockReturnValueOnce(new Promise<ListFeedbackRequestsOutput>((resolve) => {
      releaseQuery = resolve
    }))

    const loading = change === 'scope'
      ? controller.selectScope('codex', 'session-2')
      : change === 'search'
        ? controller.setRequestSearch('changed')
        : controller.setRequestFilters({ status: 'completed', timeRange: 'all' })
    expect(get(controller).loadingRequests).toBe(true)
    releaseQuery({ requests: [feedbackRequest('changed')], next_cursor: null })
    await loading
    expect(get(controller)).toMatchObject({ loadingRequests: false, requests: [feedbackRequest('changed')] })
  })

  it('keeps pagination protected until the newest background refresh finishes', async () => {
    mocks.applicationCall.mockResolvedValueOnce({ requests: [feedbackRequest('original')], next_cursor: 'old-cursor' })
    const controller = createController()
    await controller.refreshRequests()
    let releaseOlder!: (page: ListFeedbackRequestsOutput) => void
    let releaseNewest!: (page: ListFeedbackRequestsOutput) => void
    mocks.applicationCall.mockReturnValueOnce(new Promise<ListFeedbackRequestsOutput>((resolve) => {
      releaseOlder = resolve
    })).mockReturnValueOnce(new Promise<ListFeedbackRequestsOutput>((resolve) => {
      releaseNewest = resolve
    }))
    const older = controller.refreshRequests()
    const newest = controller.refreshRequests()
    releaseOlder({ requests: [feedbackRequest('stale')], next_cursor: 'stale-cursor' })
    await older
    await controller.loadMoreRequests()
    expect(mocks.applicationCall).toHaveBeenCalledTimes(3)
    expect(get(controller)).toMatchObject({ loadingRequests: false, requests: [feedbackRequest('original')] })

    releaseNewest({ requests: [feedbackRequest('newest')], next_cursor: 'new-cursor' })
    await newest
    mocks.applicationCall.mockResolvedValueOnce({ requests: [feedbackRequest('older-page')], next_cursor: null })
    await controller.loadMoreRequests()
    expect(mocks.applicationCall).toHaveBeenLastCalledWith('listFeedbackRequests', expect.objectContaining({ cursor: 'new-cursor' }))
    expect(get(controller).requests.map((request) => request.request_id)).toEqual(['newest', 'older-page'])
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
})

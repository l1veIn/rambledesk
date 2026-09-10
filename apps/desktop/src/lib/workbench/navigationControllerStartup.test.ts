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

describe('navigationController startup and facts', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    resetNavigationTestMocks()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('exposes a stalled readiness error, stops loading, and allows a later retry', async () => {
    const transport = new TestApplicationTransport()
      .resolve('listFeedbackInbox', [])
      .resolve('listHostSessions', [])
      .resolve('listHostProfiles', [])
      .resolve('listFeedbackRequests', { requests: [], next_cursor: null })
    const controller = createController({ transport })
    const first = controller.initialize(false)
    await vi.advanceTimersByTimeAsync(APPLICATION_READ_TIMEOUT_MS)
    await expect(first).resolves.toBe(false)
    expect(get(controller)).toMatchObject({ loadingNavigation: false, loadingRequests: false,
      initializationFailure: { timedOut: true, message: expect.stringContaining('application readiness') } })
    transport.markReady()
    await expect(controller.initialize(false)).resolves.toBe(true)
    expect(get(controller).initializationFailure).toBeNull()
    expect(transport.callsFor('listHostSessions')).toHaveLength(1)
  })

  it('does not let a timed-out session snapshot overwrite a successful retry', async () => {
    let resolveLate!: (sessions: HostSessionSummary[]) => void
    let first = true
    const stalled = new Promise<HostSessionSummary[]>((resolve) => { resolveLate = resolve })
    mocks.applicationCall.mockImplementation((name: string) => {
      if (name === 'listHostSessions') {
        if (first) { first = false; return stalled }
        return [hostSession({ title: 'Current retry' })]
      }
      return name === 'listFeedbackRequests' ? { requests: [], next_cursor: null } : []
    })
    const controller = createController()
    const initial = controller.initialize(false)
    await vi.advanceTimersByTimeAsync(APPLICATION_READ_TIMEOUT_MS)
    await expect(initial).resolves.toBe(false)
    expect(get(controller).initializationFailure?.timedOut).toBe(true)
    await expect(controller.initialize(false)).resolves.toBe(true)
    resolveLate([hostSession({ title: 'Old stalled result' })])
    await Promise.resolve()
    expect(get(controller).hostSessions[0]?.title).toBe('Current retry')
  })

  it('treats initial request-list errors and failed workspace activation as startup failures', async () => {
    mocks.applicationCall.mockImplementation((name: string) => {
      if (name === 'listFeedbackRequests') throw new Error('Database schema is newer than this version supports.')
      return []
    })
    const controller = createController({ openRequest: vi.fn(async () => false) })
    await expect(controller.initialize()).resolves.toBe(false)
    expect(get(controller).initializationFailure).toEqual({ timedOut: false,
      message: 'Error: Database schema is newer than this version supports.' })
    mocks.applicationCall.mockImplementation((name: string) => name === 'listFeedbackRequests'
      ? { requests: [feedbackRequest('test')], next_cursor: null } : [])
    await expect(controller.initialize()).resolves.toBe(false)
    expect(get(controller).initializationFailure?.message).toContain('initial workspace could not be opened')
    expect(get(controller).loadingNavigation).toBe(false)
  })

  it.each(['failed', 'stalled'] as const)('waits for a superseding %s invalidation instead of announcing startup success', async (failure) => {
    let resolveInitial!: (sessions: HostSessionSummary[]) => void
    const initialSessions = new Promise<HostSessionSummary[]>(resolve => { resolveInitial = resolve })
    let reads = 0
    mocks.applicationCall.mockImplementation((name: string) => {
      if (name === 'listHostSessions') {
        reads += 1
        if (reads === 1) return initialSessions
        if (failure === 'stalled') return new Promise<never>(() => {})
        throw new Error('Newer snapshot failed')
      }
      return name === 'listFeedbackRequests' ? { requests: [], next_cursor: null } : []
    })
    const controller = createController()
    const initial = controller.initialize(false)
    await vi.advanceTimersByTimeAsync(0)
    const invalidation = controller.refreshNavigation()
    resolveInitial([hostSession()])
    await vi.advanceTimersByTimeAsync(failure === 'stalled' ? APPLICATION_READ_TIMEOUT_MS : 0)
    await expect(invalidation).resolves.toBe(false)
    await expect(initial).resolves.toBe(false)
    expect(get(controller).initializationFailure?.timedOut).toBe(failure === 'stalled')
    expect(get(controller).hostSessionFactsStatus).toBe('failed')
    expect(get(controller).loadingNavigation).toBe(false)
  })

  it('still loads profiles and the first request list after a successful sessions-only invalidation wins', async () => {
    let resolveInitial!: (sessions: HostSessionSummary[]) => void
    const initialSessions = new Promise<HostSessionSummary[]>(resolve => { resolveInitial = resolve })
    let reads = 0
    const firstRequest = feedbackRequest('initial-request')
    const openRequest = vi.fn(async () => true)
    const profile = { id: 'codex', label: 'Codex', icon_svg: '', default_adapter: 'generic_mcp', continuation_mode: 'manual' }
    mocks.applicationCall.mockImplementation((name: string) => {
      if (name === 'listHostSessions') return ++reads === 1 ? initialSessions : [hostSession({ title: 'Current' })]
      if (name === 'listHostProfiles') return [profile]
      return name === 'listFeedbackRequests' ? { requests: [firstRequest], next_cursor: null } : []
    })
    const controller = createController({ openRequest })
    const initial = controller.initialize()
    await vi.advanceTimersByTimeAsync(0)
    await expect(controller.refreshNavigation(false)).resolves.toBe(true)
    resolveInitial([hostSession({ title: 'Stale' })])
    await expect(initial).resolves.toBe(true)
    expect(get(controller)).toMatchObject({ loadingNavigation: false, loadingRequests: false,
      hostProfiles: { codex: profile }, requests: [firstRequest] })
    expect(get(controller).hostSessions[0]?.title).toBe('Current')
    expect(openRequest).toHaveBeenCalledWith('initial-request', false)
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
})

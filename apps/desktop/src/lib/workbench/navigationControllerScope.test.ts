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

describe('navigationController scope selection', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    resetNavigationTestMocks()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it.each(['external', 'managed'] as const)('returns to the visible host scope after archiving a selected %s session', async (kind) => {
    const selectedSession = hostSession({ pending_count: 0, management: kind === 'external' ? { kind } : {
      kind, protocol: 'acp', agent_config_id: 'config', cwd: '/repo', remote_session_id: 'remote',
    } })
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

      expect(await controller.archiveHostSession(selectedSession)).toBe(true)

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

  it('does not report archive success while requests remain open or draft saving fails', async () => {
    const pending = createController()
    expect(await pending.archiveHostSession(hostSession())).toBe(false)
    const unsaved = createController({ isDirty: () => true, saveDraftNow: vi.fn(async () => false) })
    expect(await unsaved.archiveHostSession(hostSession({ pending_count: 0 }))).toBe(false)
    expect(mocks.applicationCall).not.toHaveBeenCalledWith('archiveHostSession', expect.anything())
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

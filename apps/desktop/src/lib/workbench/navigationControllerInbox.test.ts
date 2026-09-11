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

describe('navigationController inbox arrivals', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    resetNavigationTestMocks()
  })

  afterEach(() => {
    vi.useRealTimers()
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

  it('delivers new arrivals once, independent of OS notification permission, without replaying skipped requests', async () => {
    const old = feedbackRequest('old')
    const first = feedbackRequest('first')
    const second = feedbackRequest('second')
    let inbox = [old]
    const onRequestsArrived = vi.fn()
    mocks.applicationCall.mockImplementation(async (command: string) => {
      if (command === 'listFeedbackInbox') return inbox
      if (command === 'listHostSessions') return [hostSession()]
      if (command === 'listHostProfiles') return []
      if (command === 'listFeedbackRequests') return { requests: inbox, next_cursor: null }
    })
    const controller = createController({ onRequestsArrived })
    await controller.initialize(false)
    expect(onRequestsArrived).not.toHaveBeenCalled()
    inbox = [first, old]
    await controller.refreshNavigation()
    expect(onRequestsArrived).toHaveBeenCalledExactlyOnceWith([first])
    await controller.refreshNavigation()
    inbox = []
    await controller.refreshNavigation()
    inbox = [{ ...first, revision: 2 }, old]
    await controller.initialize(false)
    expect(onRequestsArrived).toHaveBeenCalledTimes(1)
    inbox = [second, first]
    await controller.refreshPage(0)
    expect(onRequestsArrived).toHaveBeenCalledTimes(2)
    expect(onRequestsArrived).toHaveBeenLastCalledWith([second])
    expect(mocks.notificationSend).not.toHaveBeenCalled()
  })

  it('reveals an in-scope request at the front of the list without waiting for a poll', async () => {
    const older = feedbackRequest('older')
    const arrived = { ...feedbackRequest('arrived'), host_id: older.host_id, host_session_id: older.host_session_id }
    mocks.applicationCall.mockImplementation(async (command: string) => {
      if (command === 'listFeedbackInbox') return [older]
      if (command === 'listHostSessions') return [hostSession()]
      if (command === 'listHostProfiles') return []
      if (command === 'listFeedbackRequests') return { requests: [older], next_cursor: null }
    })
    const controller = createController()
    await controller.initialize(false)
    await controller.selectScope(older.host_id, older.host_session_id)
    controller.revealRequest(arrived)
    expect(get(controller).requests.map(request => request.request_id)).toEqual(['arrived', 'older'])
    controller.revealRequest(arrived)
    expect(get(controller).requests.map(request => request.request_id)).toEqual(['arrived', 'older'])
  })
})

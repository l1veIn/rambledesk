import { get, writable } from 'svelte/store'

import type {
  FeedbackRequestSummary,
  HostSessionSummary,
  ListFeedbackRequestsInput,
  ListFeedbackRequestsOutput,
} from '../feedback'
import type { ApplicationTransport } from '../application/applicationTransport'
import { readApplicationSnapshot } from '../application/readApplicationSnapshot'
import { ApplicationReadTimeoutError, withApplicationReadTimeout } from '../application/applicationReadTimeout'
import type { WorkbenchCapabilities } from '../capabilities/workbenchCapabilities'
import { InboxNotificationTracker, playNotificationSound, type NotificationState } from '../notifications'
import { previewFixtures } from '../previewFixtures'
import {
  customNotificationSound,
  notificationPopupEnabled,
  notificationSound,
  notificationSoundEnabled,
  notificationVolume,
} from '../preferences'
import type { HostProfile } from './types'
import { filterRequestPage, requestFilterStatuses, type RequestFilters } from './requestFilters'
import { createHostSessionFacts, resolveHostProfile as resolveHostProfileFrom } from './navigation/hostSessionFacts'
import {
  now,
  requestListInput as requestListInputFrom,
  requestMatchesSearch,
  requestQueryKey as requestQueryKeyFrom,
  waitForMinimumDuration,
} from './navigation/navigationInputs'
import {
  MANUAL_PAGE_REFRESH_MIN_MS,
  initialNavigationState,
  type NavigationState,
} from './navigation/navigationTypes'

export type { NavigationState } from './navigation/navigationTypes'


type NavigationControllerContext = {
  capabilities: Pick<WorkbenchCapabilities, 'notifications' | 'tray'>
  previewMode: boolean
  transport: ApplicationTransport
  tr: (source: string, values?: Record<string, string | number>) => string
  messageFrom: (cause: unknown) => string
  getNotificationState: () => NotificationState
  getWorkspaceRequestId: () => string | undefined
  isDirty: () => boolean
  saveDraftNow: () => Promise<boolean>
  openRequest: (requestId: string, saveCurrent?: boolean) => Promise<boolean>
  clearWorkspace: () => void
  onPageError: (message: string) => void
  canSendOsBanners: () => boolean
  onRequestsArrived?: (requests: readonly FeedbackRequestSummary[]) => void
}

export type ScopeSelectionResult = Readonly<{
  selected: boolean
  requests: readonly FeedbackRequestSummary[]
}>


export function createNavigationController(context: NavigationControllerContext) {
  const store = writable<NavigationState>(initialNavigationState)
  const notificationTracker = new InboxNotificationTracker()
  let requestRefreshGeneration = 0
  let pendingRequestRefresh: number | null = null
  let displayedRequestQuery: string | null = null
  let scopeSelectionGeneration = 0

  function patch(next: Partial<NavigationState>) {
    store.update((current) => ({ ...current, ...next }))
  }

  const hostSessions = createHostSessionFacts({
    store,
    patch,
    messageFrom: context.messageFrom,
    onPageError: context.onPageError,
  })

  function requestListInput(cursor: string | null = null) {
    return requestListInputFrom(get(store), cursor)
  }

  function requestQueryKey() {
    return requestQueryKeyFrom(get(store))
  }


  async function initialize(openFirstRequest = true) {
    const refresh = hostSessions.beginRefresh()
    const hostSessionFactsIntent = refresh.generation
    let initialized = false
    context.onPageError('')
    patch({ loadingNavigation: true, loadingRequests: true, initializationFailure: null })

    if (context.previewMode) {
      displayedRequestQuery = requestQueryKey()
      patch({
        pendingRequests: previewFixtures.requests.filter(
          (request) => request.status === 'waiting' || request.status === 'in_progress',
        ),
        requests: previewFixtures.requests,
        hostProfiles: Object.fromEntries(
          previewFixtures.hostProfiles.map((profile) => [profile.id, profile]),
        ),
      })
      hostSessions.applyFacts(previewFixtures.hostSessions, hostSessionFactsIntent)
      patch({ loadingNavigation: false, loadingRequests: false })
      refresh.settle(true)
      return true
    }

    try {
      await withApplicationReadTimeout(context.transport.waitUntilReady(), 'application readiness')
      const [facts, profiles] = await Promise.all([
        Promise.all([
          readApplicationSnapshot(context.transport, 'listFeedbackInbox', undefined),
          readApplicationSnapshot(context.transport, 'listHostSessions', undefined),
        ]).then(values => ({ values }), cause => ({ cause })),
        readApplicationSnapshot(context.transport, 'listHostProfiles', undefined),
      ])
      if (!hostSessions.isCurrent(hostSessionFactsIntent)) {
        if (!await hostSessions.awaitLatest()) return false
      } else {
        if ('cause' in facts) throw facts.cause
        const [nextInbox, nextHostSessions] = facts.values
        hostSessions.applyFacts(nextHostSessions, hostSessionFactsIntent)
        applyInboxSnapshot(nextInbox)
      }
      patch({ hostProfiles: Object.fromEntries(profiles.map((profile) => [profile.id, profile])) })
      await refreshRequests(openFirstRequest, true)
      if (!hostSessions.isCurrent(hostSessionFactsIntent) && !await hostSessions.awaitLatest()) return false
      initialized = true
      return true
    } catch (cause) {
      hostSessions.failFacts(hostSessionFactsIntent)
      hostSessions.rememberFailure({
        message: context.messageFrom(cause),
        timedOut: cause instanceof ApplicationReadTimeoutError,
      })
      patch({ initializationFailure: hostSessions.failure() })
      context.onPageError(context.messageFrom(cause))
      return false
    } finally {
      refresh.settle(initialized)
      if (hostSessions.isCurrent(hostSessionFactsIntent)) {
        patch({ loadingNavigation: false })
      }
      if (pendingRequestRefresh === null) patch({ loadingRequests: false })
    }
  }

  async function refreshNavigation(refreshRequestList = false) {
    const refresh = hostSessions.beginRefresh()
    const hostSessionFactsIntent = refresh.generation
    let refreshed = false
    patch({ loadingNavigation: true })
    try {
      const [nextInbox, nextHostSessions] = await Promise.all([loadInbox(), loadHostSessions()])
      if (!hostSessions.isCurrent(hostSessionFactsIntent)) return true
      applyInboxSnapshot(nextInbox)
      hostSessions.applyFacts(nextHostSessions, hostSessionFactsIntent)
      if (refreshRequestList) await refreshRequests(false, true)
      refreshed = true
      return true
    } catch (cause) {
      if (!hostSessions.isCurrent(hostSessionFactsIntent)) return true
      hostSessions.failFacts(hostSessionFactsIntent)
      hostSessions.rememberFailure({ message: context.messageFrom(cause), timedOut: cause instanceof ApplicationReadTimeoutError })
      context.onPageError(context.messageFrom(cause))
      return false
    } finally {
      refresh.settle(refreshed)
      if (hostSessions.isCurrent(hostSessionFactsIntent)) patch({ loadingNavigation: false })
    }
  }

  async function refreshPage(minimumLoadingMs = MANUAL_PAGE_REFRESH_MIN_MS) {
    const refresh = hostSessions.beginRefresh()
    const hostSessionFactsIntent = refresh.generation
    let refreshed = false
    const requestGeneration = ++requestRefreshGeneration
    const query = requestQueryKey()
    pendingRequestRefresh = requestGeneration
    const startedAt = now()
    patch({
      loadingNavigation: true,
      loadingRequests: true,
      loadingMoreRequests: false,
      refreshingPage: true,
    })
    try {
      const [nextInbox, nextHostSessions, result] = await Promise.all([
        loadInbox(),
        loadHostSessions(),
        loadRequestList(),
      ])
      if (!hostSessions.isCurrent(hostSessionFactsIntent)) return
      applyInboxSnapshot(nextInbox)
      if (requestGeneration === requestRefreshGeneration) {
        displayedRequestQuery = query
        patch({ requests: result.requests, nextRequestCursor: result.next_cursor })
      }
      hostSessions.applyFacts(nextHostSessions, hostSessionFactsIntent)
      refreshed = true
    } catch (cause) {
      if (!hostSessions.isCurrent(hostSessionFactsIntent)) return
      hostSessions.failFacts(hostSessionFactsIntent)
      hostSessions.rememberFailure({ message: context.messageFrom(cause), timedOut: cause instanceof ApplicationReadTimeoutError })
      context.onPageError(context.messageFrom(cause))
    } finally {
      refresh.settle(refreshed)
      await waitForMinimumDuration(startedAt, minimumLoadingMs)
      if (hostSessions.isCurrent(hostSessionFactsIntent)) patch({ loadingNavigation: false })
      patch({ refreshingPage: false })
      if (requestGeneration === requestRefreshGeneration) {
        pendingRequestRefresh = null
        patch({ loadingRequests: false })
      }
    }
  }

  function applyInboxSnapshot(nextInbox: FeedbackRequestSummary[]) {
    const arrivals = notificationTracker.observe(nextInbox)
    patch({ pendingRequests: nextInbox })
    if (context.capabilities.tray.status.availability !== 'unavailable') {
      void context.capabilities.tray.implementation.setPendingCount(nextInbox.length).catch(() => {
        // Tray updates are a native convenience; the inbox remains authoritative.
      })
    }
    if (arrivals.length === 0) return
    context.onRequestsArrived?.(arrivals)

    if (
      get(notificationPopupEnabled) &&
      context.canSendOsBanners() &&
      context.getNotificationState() === 'enabled' &&
      context.capabilities.notifications.status.availability !== 'unavailable'
    ) {
      void context.capabilities.notifications.implementation.send({
        title: 'RambleDesk',
        body:
          arrivals.length === 1
            ? context.tr('A new feedback request arrived. Open the workbench to review it.')
            : context.tr('{count} new feedback requests arrived. Open the workbench to review them.', {
                count: arrivals.length,
              }),
      }).catch(() => {
        // OS banners are best-effort; the inbox remains authoritative.
      })
    }
    if (get(notificationSoundEnabled)) {
      const sound = get(notificationSound)
      void playNotificationSound(
        sound,
        get(notificationVolume),
        sound === 'custom' ? get(customNotificationSound) : null,
        context.capabilities.notifications.implementation.readCustomSound,
      )
    }
  }


  function loadInbox(): Promise<FeedbackRequestSummary[]> {
    if (context.previewMode) {
      return Promise.resolve(
        previewFixtures.requests.filter(
          (request) => request.status === 'waiting' || request.status === 'in_progress',
        ),
      )
    }
    return readApplicationSnapshot(context.transport, 'listFeedbackInbox', undefined)
  }

  function loadHostSessions(): Promise<HostSessionSummary[]> {
    if (context.previewMode) return Promise.resolve(previewFixtures.hostSessions)
    return readApplicationSnapshot(context.transport, 'listHostSessions', undefined)
  }

  async function loadRequestList(cursor: string | null = null): Promise<ListFeedbackRequestsOutput> {
    const state = get(store)
    const { timeRange, status } = state.requestFilters
    if (context.previewMode) {
      const statuses = requestFilterStatuses(status)
      return filterRequestPage(
        {
          requests:
            cursor === null
              ? previewFixtures.requests.filter(
                  (request) =>
                    (!state.selectedHostId || request.host_id === state.selectedHostId) &&
                    (!state.selectedHostSessionId ||
                      request.host_session_id === state.selectedHostSessionId) &&
                    requestMatchesSearch(request, state.requestSearch) &&
                    statuses.includes(request.status),
                )
              : [],
          next_cursor: null,
        },
        timeRange,
      )
    }
    const page = await readApplicationSnapshot(context.transport, 'listFeedbackRequests', requestListInput(cursor))
    return filterRequestPage(page, timeRange)
  }


  async function refreshRequests(
    openFirst = false,
    throwOnFailure = false,
  ): Promise<ListFeedbackRequestsOutput | null | undefined> {
    const generation = ++requestRefreshGeneration
    const query = requestQueryKey()
    pendingRequestRefresh = generation
    // Polling the displayed query should keep its rows visible and interactive.
    // Only an initial load or a different scope/search/filter needs the loading UI.
    patch({ loadingRequests: displayedRequestQuery !== query, loadingMoreRequests: false })
    try {
      const result = await loadRequestList()
      if (generation !== requestRefreshGeneration) return undefined
      displayedRequestQuery = query
      patch({ requests: result.requests, nextRequestCursor: result.next_cursor })
      const currentRequestId = context.getWorkspaceRequestId()
      if (openFirst && result.requests[0]) {
        const opened = await context.openRequest(result.requests[0].request_id, currentRequestId !== undefined)
        if (!opened && throwOnFailure) throw new Error('The initial workspace could not be opened.')
      } else if (openFirst && result.requests.length === 0) {
        if (!context.isDirty() || (await context.saveDraftNow())) context.clearWorkspace()
      }
      return result
    } catch (cause) {
      if (generation !== requestRefreshGeneration) return undefined
      if (throwOnFailure) throw cause
      context.onPageError(context.messageFrom(cause))
      return null
    } finally {
      if (generation === requestRefreshGeneration) {
        pendingRequestRefresh = null
        patch({ loadingRequests: false })
      }
    }
  }

  async function loadMoreRequests() {
    const state = get(store)
    if (
      !state.nextRequestCursor || state.loadingMoreRequests || state.loadingRequests ||
      pendingRequestRefresh !== null
    ) return
    const generation = requestRefreshGeneration
    patch({ loadingMoreRequests: true })
    try {
      const result = await loadRequestList(state.nextRequestCursor)
      if (generation !== requestRefreshGeneration) return
      const current = get(store)
      const known = new Set(current.requests.map((request) => request.request_id))
      patch({
        requests: [
          ...current.requests,
          ...result.requests.filter((request) => !known.has(request.request_id)),
        ],
        nextRequestCursor: result.next_cursor,
      })
    } catch (cause) {
      if (generation !== requestRefreshGeneration) return
      context.onPageError(context.messageFrom(cause))
    } finally {
      if (generation === requestRefreshGeneration) patch({ loadingMoreRequests: false })
    }
  }

  async function selectScope(
    hostId: string | null,
    hostSessionId: string | null,
  ): Promise<ScopeSelectionResult> {
    const generation = ++scopeSelectionGeneration
    const state = get(store)
    if (state.selectedHostId === hostId && state.selectedHostSessionId === hostSessionId) {
      return { selected: !state.loadingRequests, requests: state.requests }
    }
    if (context.isDirty() && !(await context.saveDraftNow())) {
      return { selected: false, requests: state.requests }
    }
    if (generation !== scopeSelectionGeneration) {
      return { selected: false, requests: get(store).requests }
    }
    patch({ selectedHostId: hostId, selectedHostSessionId: hostSessionId })
    const result = await refreshRequests(false)
    let current = get(store)
    if (
      result === null &&
      generation === scopeSelectionGeneration &&
      current.selectedHostId === hostId &&
      current.selectedHostSessionId === hostSessionId
    ) {
      patch({
        selectedHostId: state.selectedHostId,
        selectedHostSessionId: state.selectedHostSessionId,
        requests: state.requests,
        nextRequestCursor: state.nextRequestCursor,
      })
      current = get(store)
    }
    const selected =
      result !== null &&
      result !== undefined &&
      generation === scopeSelectionGeneration &&
      current.selectedHostId === hostId &&
      current.selectedHostSessionId === hostSessionId
    return { selected, requests: selected ? result!.requests : current.requests }
  }

  async function setRequestSearch(search: string) {
    const current = get(store).requestSearch
    if (current === search) return
    patch({ requestSearch: search, nextRequestCursor: null })
    await refreshRequests(false)
  }

  async function setRequestFilters(filters: RequestFilters) {
    const current = get(store).requestFilters
    if (current.status === filters.status && current.timeRange === filters.timeRange) return
    displayedRequestQuery = null
    patch({ requestFilters: { ...filters }, requests: [], nextRequestCursor: null })
    await refreshRequests(false)
  }

  async function renameHostSession(session: HostSessionSummary, title: string) {
    const trimmed = title.trim()
    if (!trimmed || trimmed === session.title) return
    try {
      if (context.previewMode) {
        hostSessions.replaceSession({ ...session, title: trimmed })
        return
      }
      const renamed = await context.transport.call('renameHostSession', {
        host_id: session.host_id,
        host_session_id: session.host_session_id,
        title: trimmed,
      })
      hostSessions.replaceSession(renamed)
    } catch (cause) {
      context.onPageError(context.messageFrom(cause))
      throw cause
    }
  }

  async function setHostSessionPinned(session: HostSessionSummary, pinned: boolean) {
    try {
      if (context.previewMode) {
        hostSessions.replaceSession({
          ...session,
          pinned_at: pinned ? new Date().toISOString() : null,
        })
        return
      }
      const updated = await context.transport.call('setHostSessionPinned', {
        host_id: session.host_id,
        host_session_id: session.host_session_id,
        pinned,
      })
      hostSessions.replaceSession(updated)
      await refreshNavigation(false)
    } catch (cause) {
      context.onPageError(context.messageFrom(cause))
      throw cause
    }
  }

  async function archiveHostSession(session: HostSessionSummary): Promise<boolean> {
    if (session.pending_count > 0) {
      context.onPageError(context.tr('Finish or cancel open requests before archiving this session.'))
      return false
    }
    if (context.isDirty() && !(await context.saveDraftNow())) return false
    try {
      if (!context.previewMode) {
        await context.transport.call('archiveHostSession', {
          host_id: session.host_id,
          host_session_id: session.host_session_id,
        })
      }
      const current = get(store)
      if (
        current.selectedHostId === session.host_id &&
        current.selectedHostSessionId === session.host_session_id
      ) {
        patch({ selectedHostId: session.host_id, selectedHostSessionId: null })
        context.clearWorkspace()
      }
      if (context.previewMode) {
        hostSessions.applyFacts(
          get(store).hostSessions.filter(
            (candidate) =>
              candidate.host_id !== session.host_id ||
              candidate.host_session_id !== session.host_session_id,
          ),
        )
        await refreshRequests(false)
        return true
      }
      await refreshNavigation(true)
      return true
    } catch (cause) {
      context.onPageError(context.messageFrom(cause))
      throw cause
    }
  }

  async function setHostPinned(hostId: string, pinned: boolean) {
    try {
      if (context.previewMode) {
        const pinnedAt = pinned ? new Date().toISOString() : null
        hostSessions.applyFacts(
          get(store).hostSessions.map((session) =>
            session.host_id === hostId ? { ...session, host_pinned_at: pinnedAt } : session,
          ),
        )
        return
      }
      const nextSessions = await context.transport.call('setHostPinned', {
        host_id: hostId,
        pinned,
      })
      hostSessions.applyFacts(nextSessions)
    } catch (cause) {
      context.onPageError(context.messageFrom(cause))
      throw cause
    }
  }


  return {
    subscribe: store.subscribe,
    initialize,
    refreshNavigation,
    refreshPage,
    refreshRequests,
    loadMoreRequests,
    selectScope,
    setRequestSearch,
    setRequestFilters,
    renameHostSession,
    setHostSessionPinned,
    archiveHostSession,
    setHostPinned,
    resolveHostProfile: (hostId: string) => resolveHostProfileFrom(get(store).hostProfiles, hostId),
  }
}

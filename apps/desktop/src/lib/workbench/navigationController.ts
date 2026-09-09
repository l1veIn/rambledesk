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
import {
  customNotificationSound,
  notificationPopupEnabled,
  notificationSound,
  notificationSoundEnabled,
  notificationVolume,
} from '../preferences'
import type { HostProfile } from '../domain/hostProfile'
import { filterRequestPage, requestFilterStatuses, type RequestFilters } from '../domain/requestFilters'
import { createHostSessionFacts, resolveHostProfile as resolveHostProfileFrom } from './navigation/hostSessionFacts'
import {
  now,
  requestListInput as requestListInputFrom,
  requestQueryKey as requestQueryKeyFrom,
  waitForMinimumDuration,
} from './navigation/navigationInputs'
import {
  MANUAL_PAGE_REFRESH_MIN_MS,
  initialNavigationState,
  type NavigationState,
} from './navigation/navigationTypes'
import type { PreparedNavigationScope } from './navigationScope'

export type { NavigationState } from './navigation/navigationTypes'


type NavigationControllerContext = {
  capabilities: Pick<WorkbenchCapabilities, 'notifications' | 'tray'>
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
  // Candidates carry no writable state. Only this controller can accept one,
  // and only while its query and navigation intent are still current.
  const preparedScopes = new WeakMap<PreparedNavigationScope, {
    query: string
    isCurrent: () => boolean
  }>()

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
    return readApplicationSnapshot(context.transport, 'listFeedbackInbox', undefined)
  }

  function loadHostSessions(): Promise<HostSessionSummary[]> {
    return readApplicationSnapshot(context.transport, 'listHostSessions', undefined)
  }

  async function loadRequestList(cursor: string | null = null): Promise<ListFeedbackRequestsOutput> {
    const state = get(store)
    const page = await readApplicationSnapshot(
      context.transport,
      'listFeedbackRequests',
      requestListInput(cursor),
    )
    return filterRequestPage(page, state.requestFilters.timeRange)
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

  async function readScope(
    hostId: string | null,
    hostSessionId: string | null,
    isCurrent: () => boolean,
  ): Promise<PreparedNavigationScope | null> {
    if (!isCurrent()) return null
    const current = get(store)
    const state = { ...current, selectedHostId: hostId, selectedHostSessionId: hostSessionId }
    const query = requestQueryKeyFrom(state)
    try {
      const result = query === displayedRequestQuery && !current.loadingRequests
        ? { requests: current.requests, next_cursor: current.nextRequestCursor }
        : filterRequestPage(
            await readApplicationSnapshot(context.transport, 'listFeedbackRequests', requestListInputFrom(state)),
            state.requestFilters.timeRange,
          )
      if (!isCurrent()) return null
      const prepared: PreparedNavigationScope = {
        scope: { hostId, hostSessionId },
        requests: result.requests,
        nextRequestCursor: result.next_cursor,
      }
      preparedScopes.set(prepared, { query, isCurrent })
      return prepared
    } catch (cause) {
      if (isCurrent()) context.onPageError(context.messageFrom(cause))
      return null
    }
  }

  function prepareScope(
    hostId: string | null,
    hostSessionId: string | null,
    isCurrent: () => boolean = () => true,
  ): Promise<PreparedNavigationScope | null> {
    const generation = ++scopeSelectionGeneration
    return readScope(hostId, hostSessionId, () => generation === scopeSelectionGeneration && isCurrent())
  }

  function canCommitScope(prepared: PreparedNavigationScope): boolean {
    const preparation = preparedScopes.get(prepared)
    return !!preparation && preparation.isCurrent() && preparation.query === requestQueryKeyFrom({
      ...get(store),
      selectedHostId: prepared.scope.hostId,
      selectedHostSessionId: prepared.scope.hostSessionId,
    })
  }

  function commitScope(prepared: PreparedNavigationScope): boolean {
    if (!canCommitScope(prepared)) return false
    displayedRequestQuery = preparedScopes.get(prepared)!.query
    preparedScopes.delete(prepared)
    // An older poll or pagination response belongs to the previous visible query.
    requestRefreshGeneration += 1
    pendingRequestRefresh = null
    patch({
      selectedHostId: prepared.scope.hostId,
      selectedHostSessionId: prepared.scope.hostSessionId,
      requests: [...prepared.requests],
      nextRequestCursor: prepared.nextRequestCursor,
      loadingRequests: false,
      loadingMoreRequests: false,
    })
    return true
  }

  async function selectScope(
    hostId: string | null,
    hostSessionId: string | null,
    isCurrent: () => boolean = () => true,
  ): Promise<ScopeSelectionResult> {
    const generation = ++scopeSelectionGeneration
    const current = () => generation === scopeSelectionGeneration && isCurrent()
    const state = get(store)
    if (!current()) return { selected: false, requests: state.requests }
    if (state.selectedHostId === hostId && state.selectedHostSessionId === hostSessionId && !state.loadingRequests) {
      return { selected: true, requests: state.requests }
    }
    if (context.isDirty() && !(await context.saveDraftNow())) {
      return { selected: false, requests: get(store).requests }
    }
    if (!current()) return { selected: false, requests: get(store).requests }
    patch({ loadingRequests: true })
    try {
      const prepared = await readScope(hostId, hostSessionId, current)
      const selected = prepared !== null && commitScope(prepared)
      return { selected, requests: get(store).requests }
    } finally {
      if (current()) patch({ loadingRequests: false })
    }
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
      await context.transport.call('archiveHostSession', {
        host_id: session.host_id,
        host_session_id: session.host_session_id,
      })
      const current = get(store)
      if (
        current.selectedHostId === session.host_id &&
        current.selectedHostSessionId === session.host_session_id
      ) {
        patch({ selectedHostId: session.host_id, selectedHostSessionId: null })
        context.clearWorkspace()
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
    prepareScope,
    canCommitScope,
    commitScope,
    setRequestSearch,
    setRequestFilters,
    renameHostSession,
    setHostSessionPinned,
    archiveHostSession,
    setHostPinned,
    resolveHostProfile: (hostId: string) => resolveHostProfileFrom(get(store).hostProfiles, hostId),
  }
}

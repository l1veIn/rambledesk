import { invoke } from '@tauri-apps/api/core'
import { sendNotification } from '@tauri-apps/plugin-notification'
import { get, writable } from 'svelte/store'

import type {
  FeedbackRequestSummary,
  HostSessionSummary,
  ListFeedbackRequestsInput,
  ListFeedbackRequestsOutput,
} from '../feedback'
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

const ALL_REQUEST_STATUSES = ['waiting', 'in_progress', 'completed', 'cancelled'] as const
const MANUAL_PAGE_REFRESH_MIN_MS = 300

export type NavigationState = {
  pendingRequests: FeedbackRequestSummary[]
  requests: FeedbackRequestSummary[]
  hostSessions: HostSessionSummary[]
  hostProfiles: Record<string, HostProfile>
  selectedHostId: string | null
  selectedHostSessionId: string | null
  requestSearch: string
  nextRequestCursor: string | null
  loadingNavigation: boolean
  loadingRequests: boolean
  loadingMoreRequests: boolean
  refreshingPage: boolean
}

type NavigationControllerContext = {
  isTauri: boolean
  previewMode: boolean
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
}

export type ScopeSelectionResult = Readonly<{
  selected: boolean
  requests: readonly FeedbackRequestSummary[]
}>

const initialState: NavigationState = {
  pendingRequests: [],
  requests: [],
  hostSessions: [],
  hostProfiles: {},
  selectedHostId: null,
  selectedHostSessionId: null,
  requestSearch: '',
  nextRequestCursor: null,
  loadingNavigation: true,
  loadingRequests: true,
  loadingMoreRequests: false,
  refreshingPage: false,
}

export function createNavigationController(context: NavigationControllerContext) {
  const store = writable<NavigationState>(initialState)
  const notificationTracker = new InboxNotificationTracker()
  let requestRefreshGeneration = 0
  let scopeSelectionGeneration = 0

  function patch(next: Partial<NavigationState>) {
    store.update((current) => ({ ...current, ...next }))
  }

  function replaceHostSession(summary: HostSessionSummary) {
    store.update((current) => {
      const nextSessions = current.hostSessions.map((session) =>
        session.host_id === summary.host_id &&
        session.host_session_id === summary.host_session_id
          ? summary
          : session,
      )
      if (
        !nextSessions.some(
          (session) =>
            session.host_id === summary.host_id &&
            session.host_session_id === summary.host_session_id,
        )
      ) {
        nextSessions.push(summary)
      }
      return { ...current, hostSessions: nextSessions }
    })
  }

  async function initialize() {
    context.onPageError('')
    patch({ loadingNavigation: true, loadingRequests: true })

    if (!context.isTauri) {
      if (context.previewMode) {
        patch({
          pendingRequests: previewFixtures.requests.filter(
            (request) => request.status === 'waiting' || request.status === 'in_progress',
          ),
          requests: previewFixtures.requests,
          hostSessions: previewFixtures.hostSessions,
          hostProfiles: Object.fromEntries(
            previewFixtures.hostProfiles.map((profile) => [profile.id, profile]),
          ),
        })
      }
      patch({ loadingNavigation: false, loadingRequests: false })
      return
    }

    try {
      const [nextInbox, nextHostSessions, profiles] = await Promise.all([
        invoke<FeedbackRequestSummary[]>('list_feedback_inbox'),
        invoke<HostSessionSummary[]>('list_host_sessions'),
        invoke<HostProfile[]>('list_host_profiles'),
      ])
      patch({
        hostProfiles: Object.fromEntries(profiles.map((profile) => [profile.id, profile])),
        hostSessions: nextHostSessions,
      })
      applyInboxSnapshot(nextInbox)
      await refreshRequests(true)
    } catch (cause) {
      context.onPageError(context.messageFrom(cause))
    } finally {
      patch({ loadingNavigation: false, loadingRequests: false })
    }
  }

  async function refreshNavigation(refreshRequestList = false) {
    patch({ loadingNavigation: true })
    try {
      const [nextInbox, nextHostSessions] = await Promise.all([loadInbox(), loadHostSessions()])
      applyInboxSnapshot(nextInbox)
      patch({ hostSessions: nextHostSessions })
      if (refreshRequestList) await refreshRequests(false)
    } catch (cause) {
      context.onPageError(context.messageFrom(cause))
    } finally {
      patch({ loadingNavigation: false })
    }
  }

  async function refreshPage(minimumLoadingMs = MANUAL_PAGE_REFRESH_MIN_MS) {
    const startedAt = now()
    patch({ loadingNavigation: true, loadingRequests: true, refreshingPage: true })
    try {
      const [nextInbox, nextHostSessions, result] = await Promise.all([
        loadInbox(),
        loadHostSessions(),
        loadRequestList(),
      ])
      applyInboxSnapshot(nextInbox)
      patch({
        hostSessions: nextHostSessions,
        requests: result.requests,
        nextRequestCursor: result.next_cursor,
      })
    } catch (cause) {
      context.onPageError(context.messageFrom(cause))
    } finally {
      await waitForMinimumDuration(startedAt, minimumLoadingMs)
      patch({ loadingNavigation: false, loadingRequests: false, refreshingPage: false })
    }
  }

  function applyInboxSnapshot(nextInbox: FeedbackRequestSummary[]) {
    const arrivals = notificationTracker.observe(nextInbox)
    patch({ pendingRequests: nextInbox })
    void invoke('set_pending_count', { count: nextInbox.length }).catch(() => {
      // Tray updates are a convenience; the inbox remains authoritative.
    })
    if (arrivals.length === 0) return

    if (
      get(notificationPopupEnabled) &&
      context.canSendOsBanners() &&
      context.getNotificationState() === 'enabled'
    ) {
      sendNotification({
        title: 'RambleDesk',
        body:
          arrivals.length === 1
            ? context.tr('A new feedback request arrived. Open the workbench to review it.')
            : context.tr('{count} new feedback requests arrived. Open the workbench to review them.', {
                count: arrivals.length,
              }),
      })
    }
    if (get(notificationSoundEnabled)) {
      const sound = get(notificationSound)
      void playNotificationSound(
        sound,
        get(notificationVolume),
        sound === 'custom' ? get(customNotificationSound) : null,
      )
    }
  }

  function requestListInput(cursor: string | null = null): ListFeedbackRequestsInput {
    const state = get(store)
    return {
      host_id: state.selectedHostId,
      host_session_id: state.selectedHostSessionId,
      status: [...ALL_REQUEST_STATUSES],
      archived: null,
      search: state.requestSearch.trim() || null,
      limit: 100,
      cursor,
    }
  }

  function requestMatchesSearch(request: FeedbackRequestSummary, search: string) {
    const normalized = search.trim().toLowerCase()
    if (!normalized) return true
    return [
      request.title,
      request.what_happened,
      request.source_hint,
      request.request_id,
      request.host_id,
      request.host_session_id,
    ].some((value) => (value ?? '').toLowerCase().includes(normalized))
  }

  function loadInbox(): Promise<FeedbackRequestSummary[]> {
    if (context.previewMode) {
      return Promise.resolve(
        previewFixtures.requests.filter(
          (request) => request.status === 'waiting' || request.status === 'in_progress',
        ),
      )
    }
    return invoke<FeedbackRequestSummary[]>('list_feedback_inbox')
  }

  function loadHostSessions(): Promise<HostSessionSummary[]> {
    if (context.previewMode) return Promise.resolve(previewFixtures.hostSessions)
    return invoke<HostSessionSummary[]>('list_host_sessions')
  }

  function loadRequestList(cursor: string | null = null): Promise<ListFeedbackRequestsOutput> {
    const state = get(store)
    if (context.previewMode) {
      return Promise.resolve({
        requests:
          cursor === null
            ? previewFixtures.requests.filter(
                (request) =>
                  (!state.selectedHostId || request.host_id === state.selectedHostId) &&
                  (!state.selectedHostSessionId ||
                    request.host_session_id === state.selectedHostSessionId) &&
                  requestMatchesSearch(request, state.requestSearch),
              )
            : [],
        next_cursor: null,
      })
    }
    return invoke<ListFeedbackRequestsOutput>('list_feedback_requests', {
      input: requestListInput(cursor),
    })
  }

  function now() {
    return typeof performance === 'undefined' ? Date.now() : performance.now()
  }

  async function waitForMinimumDuration(startedAt: number, minimumMs: number) {
    const remainingMs = minimumMs - (now() - startedAt)
    if (remainingMs <= 0) return
    await new Promise((resolve) => setTimeout(resolve, remainingMs))
  }

  async function refreshRequests(
    openFirst = false,
  ): Promise<ListFeedbackRequestsOutput | null | undefined> {
    const generation = ++requestRefreshGeneration
    patch({ loadingRequests: true })
    try {
      const result = await loadRequestList()
      if (generation !== requestRefreshGeneration) return undefined
      patch({ requests: result.requests, nextRequestCursor: result.next_cursor })
      const currentRequestId = context.getWorkspaceRequestId()
      if (openFirst && result.requests[0]) {
        await context.openRequest(result.requests[0].request_id, currentRequestId !== undefined)
      } else if (openFirst && result.requests.length === 0) {
        if (!context.isDirty() || (await context.saveDraftNow())) context.clearWorkspace()
      }
      return result
    } catch (cause) {
      if (generation !== requestRefreshGeneration) return undefined
      context.onPageError(context.messageFrom(cause))
      return null
    } finally {
      if (generation === requestRefreshGeneration) patch({ loadingRequests: false })
    }
  }

  async function loadMoreRequests() {
    const state = get(store)
    if (!state.nextRequestCursor || state.loadingMoreRequests) return
    patch({ loadingMoreRequests: true })
    try {
      const result = await loadRequestList(state.nextRequestCursor)
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
      context.onPageError(context.messageFrom(cause))
    } finally {
      patch({ loadingMoreRequests: false })
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

  async function renameHostSession(session: HostSessionSummary, title: string) {
    const trimmed = title.trim()
    if (!trimmed || trimmed === session.title) return
    try {
      if (context.previewMode || !context.isTauri) {
        replaceHostSession({ ...session, title: trimmed })
        return
      }
      const renamed = await invoke<HostSessionSummary>('rename_host_session', {
        input: {
          host_id: session.host_id,
          host_session_id: session.host_session_id,
          title: trimmed,
        },
      })
      replaceHostSession(renamed)
    } catch (cause) {
      context.onPageError(context.messageFrom(cause))
      throw cause
    }
  }

  async function setHostSessionPinned(session: HostSessionSummary, pinned: boolean) {
    try {
      if (context.previewMode || !context.isTauri) {
        replaceHostSession({
          ...session,
          pinned_at: pinned ? new Date().toISOString() : null,
        })
        return
      }
      const updated = await invoke<HostSessionSummary>('set_host_session_pinned', {
        input: {
          host_id: session.host_id,
          host_session_id: session.host_session_id,
          pinned,
        },
      })
      replaceHostSession(updated)
      await refreshNavigation(false)
    } catch (cause) {
      context.onPageError(context.messageFrom(cause))
      throw cause
    }
  }

  async function archiveHostSession(session: HostSessionSummary) {
    if (session.pending_count > 0) {
      context.onPageError(context.tr('Finish or cancel open requests before archiving this session.'))
      return
    }
    if (context.isDirty() && !(await context.saveDraftNow())) return
    try {
      if (!(context.previewMode || !context.isTauri)) {
        await invoke<HostSessionSummary>('archive_host_session', {
          input: {
            host_id: session.host_id,
            host_session_id: session.host_session_id,
          },
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
      if (context.previewMode || !context.isTauri) {
        patch({
          hostSessions: get(store).hostSessions.filter(
            (candidate) =>
              candidate.host_id !== session.host_id ||
              candidate.host_session_id !== session.host_session_id,
          ),
        })
        await refreshRequests(false)
        return
      }
      await refreshNavigation(true)
    } catch (cause) {
      context.onPageError(context.messageFrom(cause))
      throw cause
    }
  }

  async function setHostPinned(hostId: string, pinned: boolean) {
    try {
      if (context.previewMode || !context.isTauri) {
        const pinnedAt = pinned ? new Date().toISOString() : null
        store.update((current) => ({
          ...current,
          hostSessions: current.hostSessions.map((session) =>
            session.host_id === hostId ? { ...session, host_pinned_at: pinnedAt } : session,
          ),
        }))
        return
      }
      const nextSessions = await invoke<HostSessionSummary[]>('set_host_pinned', {
        input: { host_id: hostId, pinned },
      })
      patch({ hostSessions: nextSessions })
    } catch (cause) {
      context.onPageError(context.messageFrom(cause))
      throw cause
    }
  }

  function resolveHostProfile(hostId: string): HostProfile {
    const normalized = hostId.trim().toLowerCase()
    const profiles = get(store).hostProfiles
    const profile = profiles[normalized]
    if (profile) return profile
    return {
      id: normalized || 'generic',
      label: hostId.trim() || profiles.generic?.label || 'Generic Host',
      icon_svg: profiles.generic?.icon_svg || '',
      default_adapter: 'generic_mcp',
      continuation_mode: 'manual',
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
    renameHostSession,
    setHostSessionPinned,
    archiveHostSession,
    setHostPinned,
    resolveHostProfile,
  }
}

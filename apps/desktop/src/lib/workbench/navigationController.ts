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
  notificationPopupEnabled,
  notificationSound,
  notificationSoundEnabled,
  notificationVolume,
} from '../preferences'
import type { HostProfile } from './types'

const ALL_REQUEST_STATUSES = ['waiting', 'in_progress', 'completed', 'cancelled'] as const

export type NavigationState = {
  pendingRequests: FeedbackRequestSummary[]
  requests: FeedbackRequestSummary[]
  hostSessions: HostSessionSummary[]
  hostProfiles: Record<string, HostProfile>
  selectedHostId: string | null
  selectedHostSessionId: string | null
  nextRequestCursor: string | null
  loadingNavigation: boolean
  loadingRequests: boolean
  loadingMoreRequests: boolean
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
  openRequest: (requestId: string, saveCurrent?: boolean) => Promise<void>
  clearWorkspace: () => void
  onPageError: (message: string) => void
}

const initialState: NavigationState = {
  pendingRequests: [],
  requests: [],
  hostSessions: [],
  hostProfiles: {},
  selectedHostId: null,
  selectedHostSessionId: null,
  nextRequestCursor: null,
  loadingNavigation: true,
  loadingRequests: true,
  loadingMoreRequests: false,
}

export function createNavigationController(context: NavigationControllerContext) {
  const store = writable<NavigationState>(initialState)
  const notificationTracker = new InboxNotificationTracker()

  function patch(next: Partial<NavigationState>) {
    store.update((current) => ({ ...current, ...next }))
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
      const [nextInbox, nextHostSessions] = await Promise.all([
        invoke<FeedbackRequestSummary[]>('list_feedback_inbox'),
        invoke<HostSessionSummary[]>('list_host_sessions'),
      ])
      applyInboxSnapshot(nextInbox)
      patch({ hostSessions: nextHostSessions })
      if (refreshRequestList) await refreshRequests(false)
    } catch (cause) {
      context.onPageError(context.messageFrom(cause))
    } finally {
      patch({ loadingNavigation: false })
    }
  }

  function applyInboxSnapshot(nextInbox: FeedbackRequestSummary[]) {
    const arrivals = notificationTracker.observe(nextInbox)
    patch({ pendingRequests: nextInbox })
    void invoke('set_pending_count', { count: nextInbox.length }).catch(() => {
      // Tray updates are a convenience; the inbox remains authoritative.
    })
    if (arrivals.length === 0) return

    if (get(notificationPopupEnabled) && context.getNotificationState() === 'enabled') {
      sendNotification({
        title: 'RambleDesk',
        body:
          arrivals.length === 1
            ? context.tr('新的体验反馈请求已到达。打开工作台查看。')
            : context.tr('{count} 个新的体验反馈请求已到达。打开工作台查看。', {
                count: arrivals.length,
              }),
      })
    }
    if (get(notificationSoundEnabled)) {
      void playNotificationSound(get(notificationSound), get(notificationVolume))
    }
  }

  function requestListInput(cursor: string | null = null): ListFeedbackRequestsInput {
    const state = get(store)
    return {
      host_id: state.selectedHostId,
      host_session_id: state.selectedHostSessionId,
      status: [...ALL_REQUEST_STATUSES],
      limit: 100,
      cursor,
    }
  }

  async function refreshRequests(openFirst = false) {
    patch({ loadingRequests: true })
    try {
      const state = get(store)
      const result: ListFeedbackRequestsOutput = context.previewMode
        ? {
            requests: previewFixtures.requests.filter(
              (request) =>
                (!state.selectedHostId || request.host_id === state.selectedHostId) &&
                (!state.selectedHostSessionId ||
                  request.host_session_id === state.selectedHostSessionId),
            ),
            next_cursor: null,
          }
        : await invoke<ListFeedbackRequestsOutput>('list_feedback_requests', {
            input: requestListInput(),
          })
      patch({ requests: result.requests, nextRequestCursor: result.next_cursor })
      const currentRequestId = context.getWorkspaceRequestId()
      if (openFirst && result.requests[0]) {
        await context.openRequest(result.requests[0].request_id, currentRequestId !== undefined)
      } else if (openFirst && result.requests.length === 0) {
        if (!context.isDirty() || (await context.saveDraftNow())) context.clearWorkspace()
      }
    } catch (cause) {
      context.onPageError(context.messageFrom(cause))
    } finally {
      patch({ loadingRequests: false })
    }
  }

  async function loadMoreRequests() {
    const state = get(store)
    if (!state.nextRequestCursor || state.loadingMoreRequests) return
    patch({ loadingMoreRequests: true })
    try {
      const result = await invoke<ListFeedbackRequestsOutput>('list_feedback_requests', {
        input: requestListInput(state.nextRequestCursor),
      })
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

  async function selectScope(hostId: string | null, hostSessionId: string | null) {
    const state = get(store)
    if (state.selectedHostId === hostId && state.selectedHostSessionId === hostSessionId) return
    if (context.isDirty() && !(await context.saveDraftNow())) return
    patch({ selectedHostId: hostId, selectedHostSessionId: hostSessionId })
    await refreshRequests(false)
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
    refreshRequests,
    loadMoreRequests,
    selectScope,
    resolveHostProfile,
  }
}

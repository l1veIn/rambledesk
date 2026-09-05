import { get, writable } from 'svelte/store'
import type { ApplicationTransport } from '$lib/application/applicationTransport'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import { readApplicationSnapshot } from '$lib/application/readApplicationSnapshot'
import { applicationResourcesAffectManagedSession, createApplicationSnapshotRefetch } from '$lib/application/applicationSnapshotRefetch'
import type { ManagedFeedbackStatus, ResolveDeliveryAction } from '$lib/generated/feedback'

type StatusState = Readonly<{
  status: ManagedFeedbackStatus | null
  loading: boolean
  error: string
  resolving: boolean
  resolveError: string
}>

/** A request view observes ownership and durable delivery only, never Agent activity. */
export function createManagedFeedbackStatusController(transport: ApplicationTransport, sessionId: string, requestId: string) {
  const state = writable<StatusState>({ status: null, loading: true, error: '', resolving: false, resolveError: '' })
  let active = false
  let unsubscribe: (() => void) | null = null
  function patch(next: Partial<StatusState>) {
    if (active) state.update(current => ({ ...current, ...next }))
  }
  const refetch = createApplicationSnapshotRefetch({
    async refetch(intent) {
      await transport.waitUntilReady()
      if (!active || !intent.isCurrent()) return
      const status = await readApplicationSnapshot(transport, 'getManagedFeedbackStatus', { session_id: sessionId })
      if (status.session_id !== sessionId || status.deliveries.some(item => item.session_id !== sessionId)) {
        throw new Error('Invalid feedback status scope')
      }
      if (active && intent.isCurrent()) patch({ status, loading: false, error: '' })
    },
    reportError(cause) {
      if (typeof cause === 'object' && cause !== null && 'code' in cause && cause.code === 'MANAGED_SESSION_NOT_FOUND') {
        patch({ status: { session_id: sessionId, deleting: true, deliveries: get(state).status?.deliveries ?? [] } })
      }
      patch({ loading: false, error: 'Could not load feedback continuation status.' })
    },
  })
  function refresh() {
    if (active) refetch.request([{ kind: 'managed_session', session_id: sessionId }])
  }
  function start() {
    if (active) return dispose
    active = true
    unsubscribe = transport.subscribe(APPLICATION_EVENTS_STREAM, event => {
      if (event.type === 'ready') {
        refetch.invalidate()
        refresh()
      } else if (applicationResourcesAffectManagedSession(event.resources, sessionId)
        || event.resources.some(resource => (resource.kind === 'feedback_workspace' || resource.kind === 'published_feedback') && resource.request_id === requestId)) {
        refresh()
      }
    }, () => patch({ error: 'Could not load feedback continuation status.' }))
    refresh()
    return dispose
  }
  function dispose() {
    active = false
    unsubscribe?.()
    unsubscribe = null
    refetch.dispose()
  }
  async function resolve(action: ResolveDeliveryAction) {
    const current = get(state)
    if (!active || current.resolving || current.status?.deleting
      || !current.status?.deliveries.some(item => item.request_id === requestId && item.state === 'uncertain')) return
    patch({ resolving: true, resolveError: '' })
    try {
      // Explicit user action uses the existing command. Only a fresh lightweight
      // query updates this view; its larger mutation response is not projected.
      await transport.call('resolveFeedbackDelivery', { session_id: sessionId, request_id: requestId, action })
    } catch {
      patch({ resolveError: 'Could not update feedback continuation.' })
    } finally {
      patch({ resolving: false })
      refresh()
    }
  }
  return { subscribe: state.subscribe, start, dispose, refresh, resolve }
}

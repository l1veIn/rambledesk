import { get, writable } from 'svelte/store'
import type { ApplicationTransport } from '$lib/application/applicationTransport'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import { applicationResourcesAffectManagedSession, createApplicationSnapshotRefetch } from '$lib/application/applicationSnapshotRefetch'
import type { ManagedSessionSnapshot, ResolveDeliveryAction, SessionActivity, SessionConfigChange, SessionPromptContent } from '$lib/generated/feedback'
import { mergeActivityWindows, validateActivityPage } from './activityHistory'
import { readApplicationSnapshot } from '$lib/application/readApplicationSnapshot'

export type ManagedSessionState = Readonly<{
  snapshot: ManagedSessionSnapshot | null
  loading: boolean
  error: string
  historyLoading: boolean
  historyHasMore: boolean
  historyError: string
}>

/** One mounted workspace owns one local session ID, never the current navigation selection. */
export function createManagedSessionController(transport: ApplicationTransport, sessionId: string) {
  const state = writable<ManagedSessionState>({ snapshot: null, loading: true, error: '', historyLoading: false, historyHasMore: false, historyError: '' })
  let older: SessionActivity[] = []
  let latest: ManagedSessionSnapshot | null = null
  let historyExhausted = false
  let active = false
  let unsubscribe: (() => void) | null = null

  function patch(next: Partial<ManagedSessionState>) {
    if (active) state.update((current) => ({ ...current, ...next }))
  }

  function validate(snapshot: ManagedSessionSnapshot): ManagedSessionSnapshot {
    if (snapshot.session.session_id !== sessionId) throw new Error('The agent returned a different session.')
    return snapshot
  }

  function message(cause: unknown): string {
    return cause instanceof Error ? cause.message
      : typeof cause === 'object' && cause !== null && 'message' in cause ? String(cause.message)
        : 'Could not load the agent session.'
  }

  const refetch = createApplicationSnapshotRefetch({
    async refetch(intent) {
      await transport.waitUntilReady()
      if (!active || !intent.isCurrent()) return
      const snapshot = validate(await transport.call('getManagedSession', { session_id: sessionId }))
      if (active && intent.isCurrent()) {
        if (older.length && snapshot.activities[0]) {
          older = mergeActivityWindows(older, (get(state).snapshot?.activities ?? []).filter(row => row.sequence < snapshot.activities[0].sequence))
        }
        latest = snapshot
        projectHistory()
        patch({ loading: false, error: '' })
      }
    },
    reportError(cause) { patch({ loading: false, error: message(cause) }) },
  })

  function refresh(): void {
    if (!active) return
    patch({ loading: get(state).snapshot === null })
    refetch.request([{ kind: 'managed_session', session_id: sessionId }])
  }

  function projectHistory() {
    if (!latest) return
    const activities = mergeActivityWindows(older, latest.activities)
    patch({ snapshot: { ...latest, activities }, historyHasMore: !historyExhausted && (activities[0]?.sequence ?? 0) > 1 })
  }

  async function loadOlder(): Promise<void> {
    if (!active || get(state).historyLoading || !get(state).historyHasMore) return
    const before = get(state).snapshot?.activities[0]?.sequence
    if (!before) return
    patch({ historyLoading: true, historyError: '' })
    try {
      const page = await readApplicationSnapshot(transport, 'listManagedSessionActivity', { session_id: sessionId, before_sequence: before, limit: 100 })
      if (!active) return
      validateActivityPage(page.activities, sessionId, before)
      older = mergeActivityWindows(page.activities, older)
      historyExhausted = !page.has_more
      projectHistory()
    } catch (cause) {
      patch({ historyError: message(cause) })
    } finally { patch({ historyLoading: false }) }
  }

  function start(): () => void {
    if (active) return dispose
    active = true
    unsubscribe = transport.subscribe(APPLICATION_EVENTS_STREAM, (event) => {
      if (event.type === 'ready' || applicationResourcesAffectManagedSession(event.resources, sessionId)) refresh()
    }, (cause) => patch({ error: message(cause) }))
    refresh()
    return dispose
  }

  async function run(operation: () => Promise<ManagedSessionSnapshot>): Promise<void> {
    if (!active) throw new Error('The agent session is no longer open.')
    if (get(state).snapshot?.deleting) throw new Error('This session is being deleted. Retry deletion to finish cleanup.')
    try {
      // A mutation response acknowledges the action. Reads alone update the projection, so a
      // prompt finishing late cannot overwrite newer activity, permission, or cancellation data.
      validate(await operation())
    } finally {
      // Query the current projection after either outcome, without replaying the command.
      refresh()
    }
  }

  function dispose(): void {
    active = false
    unsubscribe?.()
    unsubscribe = null
    refetch.dispose()
    // Closing a view never changes the runtime lifetime.
  }

  return {
    subscribe: state.subscribe, start, refresh, dispose, loadOlder,
    setConfiguration: (change: SessionConfigChange) => run(() => transport.call('setManagedSessionConfig', { session_id: sessionId, change })),
    promptContent: (text: string, content: SessionPromptContent[]) => run(() => transport.call('sendManagedPromptContent', { session_id: sessionId, text, content })),
    startAgent: () => run(() => transport.call('startManagedSession', { session_id: sessionId })),
    stopAgent: () => run(() => transport.call('stopManagedSession', { session_id: sessionId })),
    cancel: () => run(() => transport.call('cancelManagedPrompt', { session_id: sessionId })),
    prompt: (text: string) => run(() => transport.call('sendManagedPrompt', { session_id: sessionId, text })),
    respondPermission: (requestId: string, optionId: string | null) => run(() => transport.call('respondManagedPermission', {
      session_id: sessionId, request_id: requestId, option_id: optionId,
    })),
    resolveDelivery: (requestId: string, action: ResolveDeliveryAction) => run(() => transport.call('resolveFeedbackDelivery', {
      session_id: sessionId, request_id: requestId, action,
    })),
  }
}

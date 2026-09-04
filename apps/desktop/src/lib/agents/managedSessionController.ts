import { get, writable } from 'svelte/store'
import type { ApplicationTransport } from '$lib/application/applicationTransport'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import { applicationResourcesAffectManagedSession, createApplicationSnapshotRefetch } from '$lib/application/applicationSnapshotRefetch'
import type { ManagedSessionSnapshot, ResolveDeliveryAction, SessionConfigChange, SessionPromptContent } from '$lib/generated/feedback'

export type ManagedSessionState = Readonly<{
  snapshot: ManagedSessionSnapshot | null
  loading: boolean
  error: string
}>

/** One mounted workspace owns one local session ID, never the current navigation selection. */
export function createManagedSessionController(transport: ApplicationTransport, sessionId: string) {
  const state = writable<ManagedSessionState>({ snapshot: null, loading: true, error: '' })
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
      if (active && intent.isCurrent()) patch({ snapshot, loading: false, error: '' })
    },
    reportError(cause) { patch({ loading: false, error: message(cause) }) },
  })

  function refresh(): void {
    if (!active) return
    patch({ loading: get(state).snapshot === null })
    refetch.request([{ kind: 'managed_session', session_id: sessionId }])
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
    subscribe: state.subscribe, start, refresh, dispose,
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

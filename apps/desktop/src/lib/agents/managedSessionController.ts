import { get, writable } from 'svelte/store'
import type { ApplicationTransport } from '$lib/application/applicationTransport'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import { applicationResourcesAffectManagedSession, createApplicationSnapshotRefetch } from '$lib/application/applicationSnapshotRefetch'
import type { ManagedSessionSnapshot, SessionActivity, SessionConfigChange } from '$lib/generated/feedback'
import { HISTORY_ACTIVITY_LIMIT, HISTORY_TURN_COUNT, completedHistoryRanges, mergeActivityWindows, retainActivityIdentity, validateActivityPage, type CompletedHistoryRange } from './activityHistory'
import { readApplicationSnapshot } from '$lib/application/readApplicationSnapshot'
import { automaticConnectionIssue, startManagedSessionOnce } from './managedSessionConnection'

export type ManagedSessionState = Readonly<{
  snapshot: ManagedSessionSnapshot | null
  loading: boolean
  error: string
  connecting: boolean
  connectionError: string
  historyLoading: boolean
  historyHasMore: boolean
  historyError: string
}>

/** One mounted workspace owns one local session ID, never the current navigation selection. */
export function createManagedSessionController(transport: ApplicationTransport, sessionId: string,
  options: Readonly<{ autoConnectBlocked?: () => boolean }> = {}) {
  const state = writable<ManagedSessionState>({ snapshot: null, loading: true, error: '', connecting: false, connectionError: '', historyLoading: false, historyHasMore: false, historyError: '' })
  let older: SessionActivity[] = []
  let latest: ManagedSessionSnapshot | null = null
  let historyExhausted = false
  let historyPrimed = false
  let completionHistory = new Map<string, CompletedHistoryRange>()
  const failedCompletionHistory = new Map<string, CompletedHistoryRange>()
  let completionHistoryTask: Promise<void> | null = null
  let completionHistoryEpoch = 0
  let active = false
  let unsubscribe: (() => void) | null = null
  let runtimeGeneration: string | null = null
  let connectionEpoch = 0
  let snapshotCurrent = false
  let autoConnectAttempted = false
  let autoConnectFailed = false
  let connectionTask: Promise<void> | null = null

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
        const completed = completedHistoryRanges(get(state).snapshot, snapshot)
        if (snapshot.activities[0]) {
          older = mergeActivityWindows(older, (get(state).snapshot?.activities ?? []).filter(row => row.sequence < snapshot.activities[0].sequence))
        }
        latest = { ...snapshot, activities: retainActivityIdentity(get(state).snapshot?.activities ?? [], snapshot.activities) }
        snapshotCurrent = true
        projectHistory()
        patch({ loading: false, error: '' })
        if (snapshot.runtime.connection === 'connected') patch({ connectionError: '' })
        ensureConnection()
        if (completed.length) void refreshCompletedHistory(completed)
        if (!historyPrimed) {
          historyPrimed = true
          // The live projection stays small. Warm scrollback once per opened view,
          // independently of subsequent streaming/status snapshot invalidations.
          if (!snapshot.deleting && snapshot.activities.filter(row => row.kind === 'user_message').length < HISTORY_TURN_COUNT) void loadOlder()
        }
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
    if (active && failedCompletionHistory.size && !get(state).historyLoading) {
      patch({ historyLoading: true, historyError: '' })
      await refreshCompletedHistory([...failedCompletionHistory.values()])
      patch({ historyLoading: false })
      return
    }
    if (!active || get(state).historyLoading || !get(state).historyHasMore) return
    const before = get(state).snapshot?.activities[0]?.sequence
    if (!before) return
    const atRequest = get(state).snapshot
    patch({ historyLoading: true, historyError: '' })
    try {
      const page = await readApplicationSnapshot(transport, 'listManagedSessionActivity', {
        session_id: sessionId, before_sequence: before, limit: HISTORY_ACTIVITY_LIMIT, turn_limit: HISTORY_TURN_COUNT,
      })
      if (!active) return
      validateActivityPage(page.activities, sessionId, before)
      older = mergeActivityWindows(page.activities, older)
      historyExhausted = !page.has_more
      projectHistory()
      // A page fetched during a turn may finish after its completion snapshot.
      // Include that newly loaded prefix in the completion repair as well.
      if (atRequest && latest) {
        const completed = completedHistoryRanges({ ...atRequest, activities: mergeActivityWindows(page.activities, atRequest.activities) }, latest)
        if (completed.length) void refreshCompletedHistory(completed)
      }
    } catch (cause) {
      patch({ historyError: message(cause) })
    } finally { patch({ historyLoading: false }) }
  }

  function refreshCompletedHistory(ranges: readonly CompletedHistoryRange[]): Promise<void> {
    if (completionHistoryTask && completionHistoryEpoch !== connectionEpoch) {
      completionHistory = new Map()
      completionHistoryTask = null
    }
    for (const range of ranges) completionHistory.set(range.turnId, range)
    if (completionHistoryTask) return completionHistoryTask
    const epoch = connectionEpoch
    completionHistoryEpoch = epoch
    const queue = completionHistory
    const current = () => active && connectionEpoch === epoch && !latest?.deleting
    const task = (async () => {
      while (queue.size && current()) {
        const range = queue.values().next().value!
        queue.delete(range.turnId)
        try {
          let before = range.before
          while (before > range.first && current()) {
            // A late page must not replace any row refreshed by a newer live read.
            const atRequest = new Map((get(state).snapshot?.activities ?? []).map(row => [row.id, row]))
            const page = await readApplicationSnapshot(transport, 'listManagedSessionActivity', {
              session_id: sessionId, before_sequence: before, limit: HISTORY_ACTIVITY_LIMIT, turn_limit: 1,
            })
            if (!current()) return
            validateActivityPage(page.activities, sessionId, before)
            const nextBefore = page.activities[0]?.sequence
            if (!nextBefore) throw new Error('Could not refresh completed work details.')
            const stored = new Map((get(state).snapshot?.activities ?? []).map(row => [row.id, row]))
            const fresh = page.activities.filter(row => row.turn_id === range.turnId && row.sequence >= range.first
              && stored.has(row.id) && stored.get(row.id) === atRequest.get(row.id))
            older = mergeActivityWindows(older, retainActivityIdentity(older, fresh))
            projectHistory()
            before = nextBefore
          }
          failedCompletionHistory.delete(range.turnId)
        } catch (cause) {
          if (!current()) return
          failedCompletionHistory.set(range.turnId, range)
          patch({ historyError: message(cause) })
        }
      }
    })().finally(() => {
      if (completionHistoryTask === task) completionHistoryTask = null
      if (!current()) queue.clear()
    })
    completionHistoryTask = task
    return task
  }

  function start(): () => void {
    if (active) return dispose
    active = true
    unsubscribe = transport.subscribe(APPLICATION_EVENTS_STREAM, (event) => {
      if (event.type === 'ready') {
        if (runtimeGeneration !== null && runtimeGeneration !== event.runtime_generation) {
          connectionEpoch += 1
          if (!autoConnectFailed) autoConnectAttempted = false
        }
        runtimeGeneration = event.runtime_generation
        snapshotCurrent = false
        refetch.invalidate()
        refresh()
      } else if (applicationResourcesAffectManagedSession(event.resources, sessionId)) refresh()
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

  function ensureConnection(): void {
    const snapshot = latest
    if (!snapshot || !active || !snapshotCurrent || snapshot.deleting || options.autoConnectBlocked?.()
      || snapshot.session.management.kind !== 'managed' || snapshot.session.lifecycle === 'prepared'
      || snapshot.runtime.activity !== 'idle') return
    if (snapshot.runtime.connection === 'connected' || snapshot.runtime.connection === 'connecting') return
    const issue = automaticConnectionIssue(snapshot)
    if (issue) { patch({ connectionError: get(state).connectionError || issue }); return }
    if (connectionTask) return
    if (!autoConnectAttempted && !autoConnectFailed) void connectAgent(false).catch(() => {})
    else patch({ connectionError: get(state).connectionError || 'Agent connection ended. Retry to reconnect.' })
  }

  function connectAgent(explicit = true): Promise<void> {
    if (!active) return Promise.reject(new Error('The agent session is no longer open.'))
    if (latest?.deleting || options.autoConnectBlocked?.()) return Promise.reject(new Error('This session is being deleted. Retry deletion to finish cleanup.'))
    if (connectionTask) return connectionTask
    if (!latest) return Promise.reject(new Error('The agent session is still loading.'))
    if (latest.runtime.connection === 'connected' || latest.runtime.connection === 'connecting') return Promise.resolve()
    patch({ connecting: true, connectionError: '' })
    const attemptEpoch = connectionEpoch
    const task = (async () => {
      try {
        await transport.waitUntilReady()
        if (!active || attemptEpoch !== connectionEpoch || !snapshotCurrent || latest?.deleting || options.autoConnectBlocked?.()) return
        if (!latest || latest.runtime.connection === 'connected' || latest.runtime.connection === 'connecting') return
        if (!explicit && (autoConnectAttempted || autoConnectFailed || automaticConnectionIssue(latest))) return
        autoConnectAttempted = true
        const result = validate(await startManagedSessionOnce(transport, sessionId))
        if (!active || attemptEpoch !== connectionEpoch) return
        if (!['connected', 'connecting'].includes(result.runtime.connection)) throw new Error(result.runtime.last_error || 'Could not connect to the agent.')
        autoConnectFailed = false
        patch({ connectionError: '' })
      } catch (cause) {
        if (!active || attemptEpoch !== connectionEpoch) return
        autoConnectFailed = true
        patch({ connectionError: message(cause) })
        throw cause
      } finally {
        connectionTask = null
        patch({ connecting: false })
        refresh()
      }
    })()
    connectionTask = task
    return task
  }

  function dispose(): void {
    active = false
    connectionEpoch += 1
    unsubscribe?.()
    unsubscribe = null
    refetch.dispose()
    // Closing a view never changes the runtime lifetime.
  }

  return {
    subscribe: state.subscribe, start, refresh, dispose, loadOlder,
    setConfiguration: (change: SessionConfigChange) => run(() => transport.call('setManagedSessionConfig', { session_id: sessionId, change })),
    startAgent: () => connectAgent(),
    cancel: () => run(() => transport.call('cancelManagedPrompt', { session_id: sessionId })),
    prompt: (text: string) => run(() => transport.call('sendManagedPrompt', { session_id: sessionId, text })),
    respondPermission: (requestId: string, optionId: string | null) => run(() => transport.call('respondManagedPermission', {
      session_id: sessionId, request_id: requestId, option_id: optionId,
    })),
  }
}

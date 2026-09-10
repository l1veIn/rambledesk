import { get, type Writable } from 'svelte/store'

import type { HostSessionSummary } from '../../feedback'
import { ApplicationReadTimeoutError, withApplicationReadTimeout } from '../../application/applicationReadTimeout'
import type { HostProfile } from '../../domain/hostProfile'
import type { NavigationState } from './navigationTypes'

type FactsRefresh = Readonly<{
  generation: number
  completion: Promise<boolean>
  settle: (ready: boolean) => void
}>

export type HostSessionFactsDeps = Readonly<{
  store: Writable<NavigationState>
  patch: (next: Partial<NavigationState>) => void
  messageFrom: (cause: unknown) => string
  onPageError: (message: string) => void
}>

/**
 * Generation-guarded host-session facts: concurrent refreshes never let a stale
 * response overwrite a newer one, and callers can await the latest completion.
 */
export function createHostSessionFacts(deps: HostSessionFactsDeps) {
  let generation = 0
  let activeRefresh: FactsRefresh | null = null
  let lastFailure: NavigationState['initializationFailure'] = null

  function beginRefresh(): FactsRefresh {
    let settle!: FactsRefresh['settle']
    const completion = new Promise<boolean>((resolve) => { settle = resolve })
    const refresh: FactsRefresh = { generation: ++generation, completion, settle }
    activeRefresh = refresh
    return refresh
  }

  async function awaitLatest(): Promise<boolean> {
    try {
      const ready = await withApplicationReadTimeout((async () => {
        for (;;) {
          const refresh = activeRefresh
          if (!refresh) return get(deps.store).hostSessionFactsStatus === 'ready'
          const result = await refresh.completion
          if (refresh === activeRefresh) return result
        }
      })(), 'current navigation refresh')
      if (!ready) deps.patch({ initializationFailure: lastFailure })
      return ready
    } catch (cause) {
      deps.patch({
        initializationFailure: {
          message: deps.messageFrom(cause),
          timedOut: cause instanceof ApplicationReadTimeoutError,
        },
      })
      deps.onPageError(deps.messageFrom(cause))
      return false
    }
  }

  function applyFacts(hostSessions: HostSessionSummary[], intent?: number): boolean {
    if (intent !== undefined && intent !== generation) return false
    if (intent === undefined) {
      generation += 1
      activeRefresh?.settle(true)
      activeRefresh = null
    }
    deps.store.update((current) => ({
      ...current,
      hostSessions,
      hostSessionFactsStatus: 'ready',
      hostSessionFactsRevision: current.hostSessionFactsRevision + 1,
    }))
    return true
  }

  function failFacts(intent: number): boolean {
    if (intent !== generation) return false
    deps.store.update((current) => ({
      ...current,
      hostSessionFactsStatus: 'failed',
      hostSessionFactsRevision: current.hostSessionFactsRevision + 1,
    }))
    return true
  }

  function replaceSession(summary: HostSessionSummary) {
    const current = get(deps.store)
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
    applyFacts(nextSessions)
  }

  function rememberFailure(failure: NavigationState['initializationFailure']) {
    lastFailure = failure
  }

  function failure(): NavigationState['initializationFailure'] {
    return lastFailure
  }

  function isCurrent(intent: number): boolean {
    return intent === generation
  }

  return {
    beginRefresh,
    awaitLatest,
    applyFacts,
    failFacts,
    replaceSession,
    rememberFailure,
    failure,
    isCurrent,
  }
}

export function resolveHostProfile(profiles: Record<string, HostProfile>, hostId: string): HostProfile {
  const normalized = hostId.trim().toLowerCase()
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

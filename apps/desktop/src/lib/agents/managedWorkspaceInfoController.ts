import { writable } from 'svelte/store'
import type { ApplicationTransport } from '$lib/application/applicationTransport'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import { readApplicationSnapshot } from '$lib/application/readApplicationSnapshot'
import type { ManagedWorkspaceInfo } from '$lib/generated/feedback'

const REFRESH_INTERVAL_MS = 15_000

/** Metadata follows the visible tab, independently of chat streaming and connection startup. */
export function createManagedWorkspaceInfoController(transport: ApplicationTransport, initialSessionId: string | null = null) {
  const state = writable<ManagedWorkspaceInfo | null>(null)
  let sessionId = initialSessionId
  let active = false
  let generation = 0
  let pending: number | null = null
  let timer: ReturnType<typeof setInterval> | undefined
  let unsubscribe: (() => void) | undefined

  async function refresh() {
    if (!active || !sessionId || pending !== null) return
    const intent = generation
    const selectedSessionId = sessionId
    pending = intent
    try {
      await transport.waitUntilReady()
      if (!active || generation !== intent) return
      const info = await readApplicationSnapshot(transport, 'getManagedWorkspaceInfo', { session_id: selectedSessionId })
      if (active && generation === intent) state.set(info)
    } catch {
      // Metadata is optional. Never keep displaying an old branch as current after a failed read.
      if (active && generation === intent) state.set(null)
    } finally {
      if (pending === intent) pending = null
    }
  }

  function setSessionId(nextSessionId: string | null) {
    if (sessionId === nextSessionId) return
    sessionId = nextSessionId
    generation += 1
    pending = null
    state.set(null)
    void refresh()
  }

  function start() {
    if (active) return dispose
    active = true
    unsubscribe = transport.subscribe(APPLICATION_EVENTS_STREAM, event => {
      if (!active || event.type !== 'ready') return
      generation += 1
      pending = null
      state.set(null)
      void refresh()
    }, () => {
      if (!active) return
      generation += 1
      pending = null
      state.set(null)
    })
    timer = setInterval(() => void refresh(), REFRESH_INTERVAL_MS)
    void refresh()
    return dispose
  }

  function dispose() {
    active = false
    generation += 1
    pending = null
    clearInterval(timer)
    unsubscribe?.()
  }

  return { subscribe: state.subscribe, start, refresh, setSessionId, dispose }
}

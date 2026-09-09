import { get, writable } from 'svelte/store'

import { resolvedRamblePhase } from './rambleSessionState'
import type {
  RamblePhase,
  VoicePhase,
} from '../domain/sessionPhases'

/**
 * Live Ramble and voice state for the workbench. `RambleSessionController` writes it
 * through bindings, the shell reads it for titles, portraits and locked interactions.
 */
export type RambleSessionState = Readonly<{
  phase: RamblePhase
  startedOnce: boolean
  requestId: string
  requestTitle: string
  message: string
  voicePhase: VoicePhase
  voiceDevice: string
  voicePartial: string
  voiceLevel: number
  voiceChunkIndex: number
  voiceModelMissing: boolean
}>

const initial: RambleSessionState = {
  phase: 'idle',
  startedOnce: false,
  requestId: '',
  requestTitle: '',
  message: '',
  voicePhase: 'idle',
  voiceDevice: '',
  voicePartial: '',
  voiceLevel: 0,
  voiceChunkIndex: 0,
  voiceModelMissing: false,
}

export type RambleSession = ReturnType<typeof createRambleSession>

export function createRambleSession() {
  const store = writable<RambleSessionState>(initial)

  /** Applies controller bindings; the workbench only reads. */
  function patch(next: Partial<RambleSessionState>) {
    store.update((current) => ({ ...current, ...next }))
  }

  /** The phase shown to the workbench, with voice listening folded in. */
  function visiblePhase(): RamblePhase {
    const state = get(store)
    return resolvedRamblePhase(state.phase, state.voicePhase)
  }

  function voiceActive(): boolean {
    const phase = get(store).voicePhase
    return phase === 'starting' || phase === 'listening' || phase === 'processing' || phase === 'stopping'
  }

  function voiceCanStop(): boolean {
    return voiceActive() || get(store).voicePhase === 'error'
  }

  function engaged(): boolean {
    return visiblePhase() !== 'idle'
  }

  function active(): boolean {
    return visiblePhase() === 'active'
  }

  /** True when the Ramble session belongs to the request the workbench shows. */
  function belongsToRequest(requestId: string | null | undefined): boolean {
    if (!engaged()) return true
    return requestId === get(store).requestId
  }

  /** Speech stop failures surface through the Ramble status line. */
  function speechStopError(): string {
    return get(store).voicePhase === 'error' ? get(store).message : ''
  }

  function reset() {
    store.set(initial)
  }

  return {
    subscribe: store.subscribe,
    /** Svelte `bind:` targets a store property, so the session keeps the store contract. */
    set: store.set,
    patch,
    visiblePhase,
    voiceActive,
    voiceCanStop,
    engaged,
    active,
    belongsToRequest,
    speechStopError,
    reset,
  }
}

import { get } from 'svelte/store'
import { describe, expect, it } from 'vitest'

import { createRambleSession } from './rambleSession'

describe('ramble session', () => {
  it('folds the voice phase into the visible Ramble phase', () => {
    const session = createRambleSession()
    expect(session.visiblePhase()).toBe('idle')

    session.patch({ voicePhase: 'listening' })
    expect(session.visiblePhase()).toBe('active')
    expect(session.voiceActive()).toBe(true)
    expect(session.engaged()).toBe(true)
    expect(session.active()).toBe(true)

    session.patch({ voicePhase: 'idle', phase: 'paused' })
    expect(session.visiblePhase()).toBe('paused')
    expect(session.active()).toBe(false)
    expect(session.engaged()).toBe(true)
  })

  it('keeps voiceCanStop through errors and reports stop failures', () => {
    const session = createRambleSession()
    session.patch({ voicePhase: 'error', message: 'microphone lost' })
    expect(session.voiceCanStop()).toBe(true)
    expect(session.speechStopError()).toBe('microphone lost')

    session.patch({ voicePhase: 'idle' })
    expect(session.voiceCanStop()).toBe(false)
    expect(session.speechStopError()).toBe('')
  })

  it('ties the session to the request it was started for', () => {
    const session = createRambleSession()
    expect(session.belongsToRequest(null)).toBe(true)

    session.patch({ phase: 'active', requestId: 'request-1', requestTitle: 'Review' })
    expect(session.belongsToRequest('request-1')).toBe(true)
    expect(session.belongsToRequest('request-2')).toBe(false)
    expect(get(session).requestTitle).toBe('Review')
  })

  it('resets every field', () => {
    const session = createRambleSession()
    session.patch({
      phase: 'active',
      startedOnce: true,
      requestId: 'request-1',
      voicePhase: 'listening',
      voiceLevel: 0.5,
    })

    session.reset()

    expect(get(session)).toMatchObject({
      phase: 'idle',
      startedOnce: false,
      requestId: '',
      voicePhase: 'idle',
      voiceLevel: 0,
    })
  })
})

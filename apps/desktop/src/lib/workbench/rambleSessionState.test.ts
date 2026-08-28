import { describe, expect, it } from 'vitest'

import { rambleRecordPresentation } from './rambleRecordButton'
import { resolvedRamblePhase } from './rambleSessionState'

describe('resolvedRamblePhase', () => {
  it('uses a live microphone session as the recording source of truth', () => {
    expect(resolvedRamblePhase('paused', 'listening')).toBe('active')
    expect(resolvedRamblePhase('idle', 'processing')).toBe('active')
    expect(resolvedRamblePhase('error', 'listening')).toBe('active')
  })

  it('keeps microphone transitions in sync with Ramble presentation', () => {
    expect(resolvedRamblePhase('paused', 'starting')).toBe('starting')
    expect(resolvedRamblePhase('active', 'stopping')).toBe('stopping')
    expect(resolvedRamblePhase('stopping', 'listening')).toBe('stopping')
  })

  it('preserves the Ramble phase when the microphone is inactive', () => {
    expect(resolvedRamblePhase('paused', 'idle')).toBe('paused')
    expect(resolvedRamblePhase('error', 'error')).toBe('error')
  })

  it('shows recording rather than resume when a paused view returns to a live session', () => {
    const phase = resolvedRamblePhase('paused', 'listening')

    expect(rambleRecordPresentation(phase, true)).toMatchObject({
      label: 'recording',
      pressed: true,
    })
  })
})

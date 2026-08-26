import { describe, expect, it } from 'vitest'

import { rambleRecordPresentation } from './rambleRecordButton'

describe('rambleRecordPresentation', () => {
  it('treats idle as a record control, not play/pause', () => {
    expect(rambleRecordPresentation('idle', false)).toEqual({
      icon: 'mic',
      label: 'start',
      variant: 'default',
      pressed: false,
    })
  })

  it('shows a spinner while the microphone is starting or stopping', () => {
    expect(rambleRecordPresentation('starting', false).icon).toBe('spinner')
    expect(rambleRecordPresentation('starting', false).label).toBe('starting')
    expect(rambleRecordPresentation('stopping', true)).toMatchObject({
      icon: 'spinner',
      label: 'stopping',
      pressed: false,
    })
  })

  it('uses a blinking record state while capturing', () => {
    expect(rambleRecordPresentation('active', true)).toEqual({
      icon: 'recording',
      label: 'recording',
      variant: 'destructive',
      pressed: true,
    })
  })

  it('resumes from pause and error with the mic icon', () => {
    expect(rambleRecordPresentation('paused', true).label).toBe('resume')
    expect(rambleRecordPresentation('error', true).label).toBe('resume')
    expect(rambleRecordPresentation('idle', true).label).toBe('resume')
  })
})

import type { RamblePhase } from './types'

export type RambleRecordIcon = 'spinner' | 'recording' | 'mic'
export type RambleRecordLabel = 'starting' | 'stopping' | 'recording' | 'resume' | 'start'

export type RambleRecordPresentation = {
  icon: RambleRecordIcon
  label: RambleRecordLabel
  variant: 'default' | 'destructive'
  pressed: boolean
}

export function rambleRecordPresentation(
  phase: RamblePhase,
  startedOnce: boolean,
): RambleRecordPresentation {
  if (phase === 'starting') {
    return { icon: 'spinner', label: 'starting', variant: 'default', pressed: false }
  }
  if (phase === 'stopping') {
    return { icon: 'spinner', label: 'stopping', variant: 'default', pressed: false }
  }
  if (phase === 'active') {
    return { icon: 'recording', label: 'recording', variant: 'destructive', pressed: true }
  }
  return {
    icon: 'mic',
    label: startedOnce || phase === 'paused' || phase === 'error' ? 'resume' : 'start',
    variant: 'default',
    pressed: false,
  }
}

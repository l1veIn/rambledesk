import type { RamblePhase, VoicePhase } from './types'

/**
 * The microphone session is the source of truth while it is transitioning or
 * capturing. This keeps presentation state from drifting after a view remount
 * or a delayed bound-property update.
 */
export function resolvedRamblePhase(
  ramblePhase: RamblePhase,
  voicePhase: VoicePhase,
): RamblePhase {
  if (ramblePhase === 'stopping' || voicePhase === 'stopping') return 'stopping'
  if (voicePhase === 'starting') return 'starting'
  if (voicePhase === 'listening' || voicePhase === 'processing') return 'active'
  return ramblePhase
}

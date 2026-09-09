import type {
  RamblePhase,
  VoicePhase,
} from '../domain/sessionPhases'

/**
 * The microphone session is the source of truth while it is transitioning or
 * capturing. The shared Ramble projection reads these facts directly, including while
 * the surrounding presentation changes.
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

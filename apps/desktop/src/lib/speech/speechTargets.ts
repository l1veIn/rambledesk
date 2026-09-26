/**
 * Speech persists the same destination contract as other input methods.
 * Keep these names for the speech queue and its versioned recovery records.
 */
export {
  normalizeInputTarget as normalizeSpeechTarget,
  sameInputTarget as sameSpeechTarget,
  snapshotInputTarget as snapshotSpeechTarget,
  type InputDestination as SpeechDestination,
  type InputTarget as SpeechTarget,
} from '../domain/inputTarget'

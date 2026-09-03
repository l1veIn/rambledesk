import type { SpeechRecognitionEvent } from '../speech'
import type { SpeechTarget } from './speechDraftQueue'

/** Non-streaming VAD pins at speech onset; streaming pins at the first
 * partial. Raw microphone volume never decides where words belong. */
export function createSpeechTargetTracker(capture: () => SpeechTarget) {
  let streamingTarget: SpeechTarget | null = null
  const targets = new Map<number, SpeechTarget>()
  const snapshot = () => {
    const target = capture()
    return { ...target, action: target.action ? { ...target.action } : null }
  }
  return {
    observe(event: SpeechRecognitionEvent): SpeechTarget | null {
      if (event.type === 'speech-started') {
        if (!targets.has(event.segmentIndex)) targets.set(event.segmentIndex, snapshot())
      } else if (event.type === 'partial' && event.text.trim()) {
        streamingTarget ??= snapshot()
      } else if (event.type === 'processing') {
        if (!targets.has(event.segmentIndex)) targets.set(event.segmentIndex, streamingTarget ?? snapshot())
        streamingTarget = null
      } else if (event.type === 'stable') {
        const pinned = targets.get(event.segmentIndex)
        targets.delete(event.segmentIndex)
        if (pinned) return pinned
        const target = streamingTarget ?? snapshot()
        streamingTarget = null
        return target
      }
      return null
    },
    reset() { streamingTarget = null; targets.clear() },
  }
}

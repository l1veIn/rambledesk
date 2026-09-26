import type { SpeechRecognitionEvent } from './speech'
import { snapshotSpeechTarget, type SpeechTarget } from './speechTargets'

/** Non-streaming VAD pins at speech onset; streaming pins at the first
 * partial. Raw microphone volume never decides where words belong. */
export function createSpeechTargetTracker(capture: () => SpeechTarget) {
  let streamingTarget: SpeechTarget | null = null
  let active: { target: SpeechTarget; segmentIndex?: number } | null = null
  const targets = new Map<number, SpeechTarget>()
  const snapshot = () => snapshotSpeechTarget(capture())
  return {
    activeTarget: (): SpeechTarget | null => active ? snapshotSpeechTarget(active.target) : null,
    observe(event: SpeechRecognitionEvent): SpeechTarget | null {
      if (event.type === 'speech-started') {
        if (!targets.has(event.segmentIndex)) targets.set(event.segmentIndex, snapshot())
        active = { target: targets.get(event.segmentIndex)!, segmentIndex: event.segmentIndex }
      } else if (event.type === 'partial' && event.text.trim()) {
        streamingTarget ??= active?.target ?? snapshot()
        active ??= { target: streamingTarget }
      } else if (event.type === 'processing') {
        if (!targets.has(event.segmentIndex)) targets.set(event.segmentIndex, streamingTarget ?? snapshot())
        streamingTarget = null
        if (active?.segmentIndex === undefined || active.segmentIndex === event.segmentIndex) active = null
      } else if (event.type === 'stable') {
        const pinned = targets.get(event.segmentIndex)
        targets.delete(event.segmentIndex)
        if (active?.segmentIndex === event.segmentIndex) { active = null; streamingTarget = null }
        if (pinned) return pinned
        const target = streamingTarget ?? snapshot()
        streamingTarget = null
        active = null
        return target
      }
      return null
    },
    reset() { streamingTarget = null; active = null; targets.clear() },
  }
}

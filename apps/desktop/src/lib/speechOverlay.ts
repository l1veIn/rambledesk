import type { VoicePhase } from './workbench/types'
import type { ShortcutConfig } from './shortcutSettings'
import type { RambleConsoleCommand } from './rambleConsole'
import type { SpeechDraftGroup, SpeechReceipt, SpeechTarget } from './workbench/speechDraftQueue'

export const SPEECH_OVERLAY_LABEL = 'speech-overlay'
export const SPEECH_OVERLAY_STATE_EVENT = 'speech-overlay-state'
export const SPEECH_OVERLAY_READY_EVENT = 'speech-overlay-ready'

export type SpeechOverlayState = {
  enabled: boolean
  opacity: number
  selectedGroupId: string | null
  shortcuts: Pick<ShortcutConfig, 'speechAccept' | 'speechDiscard'>
  phase: VoicePhase
  level: number
  partial: string
  error: string
  target: SpeechTarget | null
  groups: SpeechDraftGroup[]
  receipt: SpeechReceipt | null
}

export function speechOverlayVisible(state: SpeechOverlayState): boolean {
  return state.enabled && (state.phase !== 'idle' || state.groups.length > 0 || state.receipt !== null)
}

export function selectedSpeechGroup(state: Pick<SpeechOverlayState, 'groups' | 'selectedGroupId'>): SpeechDraftGroup | undefined {
  return state.groups.find((group) => group.ids.includes(state.selectedGroupId ?? '')) ?? state.groups[0]
}

export function speechReviewCommand(state: SpeechOverlayState, action: 'accept' | 'discard', locked = false): RambleConsoleCommand | null {
  const group = selectedSpeechGroup(state)
  if (!group || group.busy || locked) return null
  return { type: action === 'accept' ? 'accept-speech' : 'discard-speech', ids: [...group.ids] }
}

import type { ShortcutCapability } from '../workbenchCapabilities'
import { subscribeToTauriEvent } from './subscription'
import type { TauriCapabilityApi } from './tauriCapabilityApi'

export function createTauriShortcutCapability(api: TauriCapabilityApi): ShortcutCapability {
  return {
    read: () => api.invoke('get_shortcut_settings'),
    update: (action, shortcut) =>
      api.invoke('set_shortcut_setting', { action, shortcut }),
    reset: () => api.invoke('reset_shortcut_settings'),
    setCaptureActive: (active) =>
      api.invoke<void>('set_shortcut_capture_active', { active }),
    setSpeechReviewActive: (active) =>
      api.invoke<void>('set_speech_review_shortcuts_active', { active }),
    onSpeechReview: (handler, onError) =>
      subscribeToTauriEvent(api, 'speech-review-shortcut', handler, onError),
    onRambleToggle: (handler, onError) =>
      subscribeToTauriEvent(api, 'ramble-toggle-shortcut', handler, onError),
  }
}

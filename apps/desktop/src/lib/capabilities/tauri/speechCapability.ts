import type {
  SpeechAdministrationCapability,
  SpeechCapability,
} from '../workbenchCapabilities'
import { subscribeToTauriEvent } from './subscription'
import type { TauriCapabilityApi } from './tauriCapabilityApi'

export function createTauriSpeechCapability(
  api: TauriCapabilityApi,
): SpeechCapability & SpeechAdministrationCapability {
  return {
    start: (input) =>
      api.invoke('start_voice_ramble', {
        input: {
          request_id: input.requestId,
          input_device: input.inputDevice,
          model_id: input.modelId,
          vad_threshold: input.vadThreshold,
          vad_silence_ms: input.vadSilenceMs,
          hotwords: [...input.hotwords],
        },
      }),
    stop: () => api.invoke<void>('stop_voice_ramble'),
    onEvent: (handler, onError) =>
      subscribeToTauriEvent(api, 'voice-ramble-event', handler, onError),
    listModels: () => api.invoke('list_speech_models'),
    downloadModel: (modelId) => api.invoke('download_speech_model', { modelId }),
    deleteModel: (modelId) => api.invoke('delete_speech_model', { modelId }),
    listInputDevices: () => api.invoke('list_speech_input_devices'),
    onModelProgress: (handler, onError) =>
      subscribeToTauriEvent(api, 'speech-model-progress', handler, onError),
  }
}

import { createWorkbenchCapabilities, type MacPermission, type NotificationPermission, type SpeechModelInfo, type SpeechModelProgress } from '$lib/capabilities/workbenchCapabilities'
import { DEFAULT_SPEECH_MODEL_ID } from '$lib/preferences'
import { adapterPreviewCapabilities } from './adapterPreviewCapabilities'

/** In-memory onboarding fixtures; never request OS permissions or download real models. */
export function createOnboardingPreviewCapabilities(platform: 'Windows' | 'macOS') {
  const base = adapterPreviewCapabilities
  const status = { availability: 'available' as const, source: 'native' as const }
  const progressListeners = new Set<(progress: SpeechModelProgress) => void>()
  let notificationPermission: NotificationPermission = 'default'
  let storagePath = platform === 'macOS' ? '/Users/preview/Library/Application Support/RambleDesk' : 'D:/Preview/RambleDesk'
  let permissions: MacPermission[] = ['screen_capture', 'microphone'].map(id => ({ id, status: 'not_determined', restart_required: false }))
  let models: SpeechModelInfo[] = [
    { id: DEFAULT_SPEECH_MODEL_ID, engine_id: 'sherpa_offline', display_name: 'SenseVoice', description: '本地多语言语音识别（模拟模型）', size_bytes: 228_000_000, installed: false, path: '', missing_files: ['model.int8.onnx'], streaming: false, hotwords_supported: false, languages: ['中文', 'English', '日本語', '한국어', '粤语'], license: 'Apache-2.0' },
    { id: 'x-asr-480ms-streaming-zh-en-punct-int8-2026-06-05', engine_id: 'sherpa_online', display_name: 'X-ASR', description: '本地流式语音识别（模拟模型）', size_bytes: 180_000_000, installed: true, path: '/preview/models/x-asr', missing_files: [], streaming: true, hotwords_supported: true, languages: ['中文', 'English'], license: 'Apache-2.0' },
  ]
  return createWorkbenchCapabilities({
    ...base,
    windowControls: { status, implementation: { ...base.windowControls.implementation, platform: () => platform, onFocusChanged: () => () => {}, restart: async () => { throw new Error('隔离预览不会重启应用。') } } },
    serverPaths: { status, implementation: { ...base.serverPaths.implementation, chooseDirectory: async () => platform === 'macOS' ? '/Users/preview/RambleDesk' : 'D:/Preview/NewRambleDesk' } },
    dataStorageAdministration: { status, implementation: {
      read: async () => ({ active_path: storagePath, selected_path: storagePath, restart_required: false }),
      select: async path => { storagePath = path; return { active_path: path, selected_path: path, restart_required: false } },
      onProgress: () => () => {},
    } },
    speech: { status, implementation: {
      ...base.speech.implementation,
      listModels: async () => models,
      listInputDevices: async () => ['Preview microphone'],
      onModelProgress: handler => { progressListeners.add(handler); return () => { progressListeners.delete(handler) } },
      downloadModel: async modelId => {
        const model = models.find(item => item.id === modelId)
        if (!model) throw new Error('Unknown preview model')
        for (const fraction of [0.2, 0.6, 1]) {
          await new Promise(resolve => setTimeout(resolve, 150))
          for (const listener of progressListeners) listener({ model_id: modelId, downloaded: model.size_bytes * fraction, total: model.size_bytes })
        }
        const installed = { ...model, installed: true, path: `/preview/models/${modelId}`, missing_files: [] }
        models = models.map(item => item.id === modelId ? installed : item)
        return installed
      },
    } },
    notifications: { status, implementation: {
      ...base.notifications.implementation,
      permission: async () => notificationPermission,
      requestPermission: async () => { notificationPermission = 'granted'; return notificationPermission },
    } },
    systemPermissions: platform === 'macOS' ? { status, implementation: {
      list: async () => permissions,
      request: async id => {
        const permission: MacPermission = { id, status: 'granted', restart_required: false }
        permissions = permissions.map(item => item.id === id ? permission : item)
        return permission
      },
      openSettings: async () => {},
    } } : base.systemPermissions,
  })
}

import { get, writable } from 'svelte/store'

export type Locale = 'zh-CN' | 'en'
export type ThemePreference = 'system' | 'light' | 'dark'
export type NotificationSound = 'chime' | 'soft' | 'alert'
export type CookingProvider = 'deepseek' | 'openai' | 'compatible'
export type CookingReasoningEffort =
  | 'none'
  | 'minimal'
  | 'low'
  | 'medium'
  | 'high'
  | 'xhigh'
  | 'max'
export type SpeechModelId =
  | 'x-asr-480ms-streaming-zh-en-punct-int8-2026-06-05'
  | 'sense-voice-zh-en-ja-ko-yue-2024-07-17'
  | 'funasr-nano-int8-2025-12-30'

export const DEFAULT_SPEECH_MODEL_ID: SpeechModelId =
  'x-asr-480ms-streaming-zh-en-punct-int8-2026-06-05'

const LOCALE_KEY = 'rambledesk.locale'
const THEME_KEY = 'rambledesk.theme'
const NOTIFICATION_POPUP_KEY = 'rambledesk.notifications.popup'
const NOTIFICATION_SOUND_ENABLED_KEY = 'rambledesk.notifications.sound-enabled'
const NOTIFICATION_SOUND_KEY = 'rambledesk.notifications.sound'
const NOTIFICATION_VOLUME_KEY = 'rambledesk.notifications.volume'
const SPEECH_INPUT_DEVICE_KEY = 'rambledesk.speech.input-device'
const SPEECH_MODEL_KEY = 'rambledesk.speech.model'
const SPEECH_VAD_THRESHOLD_KEY = 'rambledesk.speech.vad-threshold'
const SPEECH_VAD_SILENCE_MS_KEY = 'rambledesk.speech.vad-silence-ms'
const COOKING_ENABLED_KEY = 'rambledesk.cooking.enabled'
const COOKING_PROVIDER_KEY = 'rambledesk.cooking.provider'
const COOKING_API_KEY_KEY = 'rambledesk.cooking.api-key'
const COOKING_BASE_URL_KEY = 'rambledesk.cooking.base-url'
const COOKING_MODEL_KEY = 'rambledesk.cooking.model'
const COOKING_REASONING_EFFORT_KEY = 'rambledesk.cooking.reasoning-effort'
const ONBOARDING_COMPLETED_KEY = 'rambledesk.onboarding.completed'
const ONBOARDING_STEP_KEY = 'rambledesk.onboarding.step'

function initialLocale(): Locale {
  const saved = localStorage.getItem(LOCALE_KEY)
  if (saved === 'zh-CN' || saved === 'en') return saved
  return navigator.language.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en'
}

function initialTheme(): ThemePreference {
  const saved = localStorage.getItem(THEME_KEY)
  return saved === 'light' || saved === 'dark' || saved === 'system'
    ? saved
    : 'system'
}

function initialBoolean(key: string, fallback: boolean) {
  const saved = localStorage.getItem(key)
  if (saved === 'true') return true
  if (saved === 'false') return false
  return fallback
}

function initialOnboardingCompleted() {
  const saved = localStorage.getItem(ONBOARDING_COMPLETED_KEY)
  if (saved === 'true') return true
  if (saved === 'false') return false
  // Existing installations already have preference state. Do not interrupt them on upgrade.
  return localStorage.getItem(LOCALE_KEY) !== null
}

function initialNotificationSound(): NotificationSound {
  const saved = localStorage.getItem(NOTIFICATION_SOUND_KEY)
  return saved === 'chime' || saved === 'soft' || saved === 'alert' ? saved : 'chime'
}

function initialNotificationVolume() {
  const raw = localStorage.getItem(NOTIFICATION_VOLUME_KEY)
  const saved = raw === null ? Number.NaN : Number(raw)
  return Number.isFinite(saved) && saved >= 0 && saved <= 100 ? saved : 100
}

function isSpeechModelId(value: string | null): value is SpeechModelId {
  return (
    value === 'x-asr-480ms-streaming-zh-en-punct-int8-2026-06-05' ||
    value === 'sense-voice-zh-en-ja-ko-yue-2024-07-17' ||
    value === 'funasr-nano-int8-2025-12-30'
  )
}

function initialNumber(key: string, fallback: number, minimum: number, maximum: number) {
  const raw = localStorage.getItem(key)
  const saved = raw === null ? Number.NaN : Number(raw)
  return Number.isFinite(saved) && saved >= minimum && saved <= maximum ? saved : fallback
}

function initialCookingProvider(): CookingProvider {
  const saved = localStorage.getItem(COOKING_PROVIDER_KEY)
  return saved === 'openai' || saved === 'compatible' || saved === 'deepseek'
    ? saved
    : 'deepseek'
}

function isCookingReasoningEffort(value: string | null): value is CookingReasoningEffort {
  return (
    value === 'none' ||
    value === 'minimal' ||
    value === 'low' ||
    value === 'medium' ||
    value === 'high' ||
    value === 'xhigh' ||
    value === 'max'
  )
}

export const locale = writable<Locale>(initialLocale())
export const themePreference = writable<ThemePreference>(initialTheme())
export const notificationPopupEnabled = writable(initialBoolean(NOTIFICATION_POPUP_KEY, true))
export const notificationSoundEnabled = writable(
  initialBoolean(NOTIFICATION_SOUND_ENABLED_KEY, true),
)
export const notificationSound = writable<NotificationSound>(initialNotificationSound())
export const notificationVolume = writable(initialNotificationVolume())
export const speechInputDevice = writable(localStorage.getItem(SPEECH_INPUT_DEVICE_KEY) ?? '')
const savedSpeechModel = localStorage.getItem(SPEECH_MODEL_KEY)
export const speechModelId = writable<SpeechModelId>(
  isSpeechModelId(savedSpeechModel) ? savedSpeechModel : DEFAULT_SPEECH_MODEL_ID,
)
export const speechVadThreshold = writable(
  initialNumber(SPEECH_VAD_THRESHOLD_KEY, 0.5, 0.05, 0.95),
)
export const speechVadSilenceMs = writable(
  initialNumber(SPEECH_VAD_SILENCE_MS_KEY, 700, 200, 5000),
)
export const onboardingCompleted = writable(initialOnboardingCompleted())
export const cookingEnabled = writable(initialBoolean(COOKING_ENABLED_KEY, false))
export const cookingProvider = writable<CookingProvider>(initialCookingProvider())
export const cookingApiKey = writable(localStorage.getItem(COOKING_API_KEY_KEY) ?? '')
export const cookingBaseUrl = writable(
  localStorage.getItem(COOKING_BASE_URL_KEY) ?? 'https://api.deepseek.com/v1',
)
export const cookingModel = writable(
  localStorage.getItem(COOKING_MODEL_KEY) ?? 'deepseek-v4-flash',
)
const savedCookingReasoningEffort = localStorage.getItem(COOKING_REASONING_EFFORT_KEY)
export const cookingReasoningEffort = writable<CookingReasoningEffort>(
  isCookingReasoningEffort(savedCookingReasoningEffort) ? savedCookingReasoningEffort : 'max',
)

let initialized = false
let mediaQuery: MediaQueryList | null = null

function applyTheme(preference: ThemePreference) {
  const dark =
    preference === 'dark' ||
    (preference === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches)
  document.documentElement.dataset.theme = dark ? 'dark' : 'light'
  document.documentElement.style.colorScheme = dark ? 'dark' : 'light'
}

export function setLocale(next: Locale) {
  locale.set(next)
}

export function setThemePreference(next: ThemePreference) {
  themePreference.set(next)
}

export function setNotificationPopupEnabled(enabled: boolean) {
  notificationPopupEnabled.set(enabled)
}

export function setNotificationSoundEnabled(enabled: boolean) {
  notificationSoundEnabled.set(enabled)
}

export function setNotificationSound(sound: NotificationSound) {
  notificationSound.set(sound)
}

export function setSpeechInputDevice(device: string) {
  speechInputDevice.set(device)
}

export function setSpeechModelId(modelId: SpeechModelId) {
  speechModelId.set(modelId)
}

export function setSpeechVadThreshold(threshold: number) {
  speechVadThreshold.set(Math.min(0.95, Math.max(0.05, threshold)))
}

export function setSpeechVadSilenceMs(milliseconds: number) {
  speechVadSilenceMs.set(Math.min(5000, Math.max(200, Math.round(milliseconds))))
}

export function setOnboardingCompleted(completed: boolean) {
  onboardingCompleted.set(completed)
}

export function onboardingStep() {
  const value = Number(localStorage.getItem(ONBOARDING_STEP_KEY))
  return Number.isInteger(value) && value >= 0 ? value : 0
}

export function setOnboardingStep(step: number) {
  localStorage.setItem(ONBOARDING_STEP_KEY, String(Math.max(0, Math.floor(step))))
}

export function resetOnboarding() {
  localStorage.removeItem(ONBOARDING_STEP_KEY)
  setOnboardingCompleted(false)
}

export function finishOnboarding() {
  localStorage.removeItem(ONBOARDING_STEP_KEY)
  setOnboardingCompleted(true)
}

export function setCookingEnabled(enabled: boolean) {
  cookingEnabled.set(enabled)
}

export function setCookingProvider(provider: CookingProvider) {
  cookingProvider.set(provider)
}

export function setCookingApiKey(apiKey: string) {
  cookingApiKey.set(apiKey)
}

export function setCookingBaseUrl(baseUrl: string) {
  cookingBaseUrl.set(baseUrl)
}

export function setCookingModel(model: string) {
  cookingModel.set(model)
}

export function setCookingReasoningEffort(effort: CookingReasoningEffort) {
  cookingReasoningEffort.set(effort)
}

export function setNotificationVolume(volume: number) {
  notificationVolume.set(Math.min(100, Math.max(0, Math.round(volume))))
}

export function initializePreferences() {
  if (initialized) return
  initialized = true

  locale.subscribe((next) => {
    localStorage.setItem(LOCALE_KEY, next)
    document.documentElement.lang = next
  })
  themePreference.subscribe((next) => {
    localStorage.setItem(THEME_KEY, next)
    applyTheme(next)
  })
  notificationPopupEnabled.subscribe((next) => {
    localStorage.setItem(NOTIFICATION_POPUP_KEY, String(next))
  })
  notificationSoundEnabled.subscribe((next) => {
    localStorage.setItem(NOTIFICATION_SOUND_ENABLED_KEY, String(next))
  })
  notificationSound.subscribe((next) => {
    localStorage.setItem(NOTIFICATION_SOUND_KEY, next)
  })
  notificationVolume.subscribe((next) => {
    localStorage.setItem(NOTIFICATION_VOLUME_KEY, String(next))
  })
  speechInputDevice.subscribe((next) => {
    localStorage.setItem(SPEECH_INPUT_DEVICE_KEY, next)
  })
  speechModelId.subscribe((next) => {
    localStorage.setItem(SPEECH_MODEL_KEY, next)
  })
  speechVadThreshold.subscribe((next) => {
    localStorage.setItem(SPEECH_VAD_THRESHOLD_KEY, String(next))
  })
  speechVadSilenceMs.subscribe((next) => {
    localStorage.setItem(SPEECH_VAD_SILENCE_MS_KEY, String(next))
  })
  onboardingCompleted.subscribe((next) => {
    localStorage.setItem(ONBOARDING_COMPLETED_KEY, String(next))
  })
  cookingEnabled.subscribe((next) => {
    localStorage.setItem(COOKING_ENABLED_KEY, String(next))
  })
  cookingProvider.subscribe((next) => {
    localStorage.setItem(COOKING_PROVIDER_KEY, next)
  })
  cookingApiKey.subscribe((next) => {
    localStorage.setItem(COOKING_API_KEY_KEY, next)
  })
  cookingBaseUrl.subscribe((next) => {
    localStorage.setItem(COOKING_BASE_URL_KEY, next)
  })
  cookingModel.subscribe((next) => {
    localStorage.setItem(COOKING_MODEL_KEY, next)
  })
  cookingReasoningEffort.subscribe((next) => {
    localStorage.setItem(COOKING_REASONING_EFFORT_KEY, next)
  })

  mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
  mediaQuery.addEventListener('change', () => {
    if (get(themePreference) === 'system') applyTheme('system')
  })
  window.addEventListener('storage', (event) => {
    if (event.key === LOCALE_KEY && (event.newValue === 'zh-CN' || event.newValue === 'en')) {
      locale.set(event.newValue)
    }
    if (
      event.key === THEME_KEY &&
      (event.newValue === 'system' || event.newValue === 'light' || event.newValue === 'dark')
    ) {
      themePreference.set(event.newValue)
    }
    if (event.key === NOTIFICATION_POPUP_KEY && event.newValue !== null) {
      notificationPopupEnabled.set(event.newValue === 'true')
    }
    if (event.key === NOTIFICATION_SOUND_ENABLED_KEY && event.newValue !== null) {
      notificationSoundEnabled.set(event.newValue === 'true')
    }
    if (
      event.key === NOTIFICATION_SOUND_KEY &&
      (event.newValue === 'chime' || event.newValue === 'soft' || event.newValue === 'alert')
    ) {
      notificationSound.set(event.newValue)
    }
    if (event.key === NOTIFICATION_VOLUME_KEY && event.newValue !== null) {
      setNotificationVolume(Number(event.newValue))
    }
    if (event.key === SPEECH_MODEL_KEY && isSpeechModelId(event.newValue)) {
      speechModelId.set(event.newValue)
    }
    if (event.key === SPEECH_VAD_THRESHOLD_KEY && event.newValue !== null) {
      setSpeechVadThreshold(Number(event.newValue))
    }
    if (event.key === SPEECH_VAD_SILENCE_MS_KEY && event.newValue !== null) {
      setSpeechVadSilenceMs(Number(event.newValue))
    }
    if (event.key === ONBOARDING_COMPLETED_KEY && event.newValue !== null) {
      onboardingCompleted.set(event.newValue === 'true')
    }
    if (event.key === COOKING_ENABLED_KEY && event.newValue !== null) {
      cookingEnabled.set(event.newValue === 'true')
    }
    if (
      event.key === COOKING_PROVIDER_KEY &&
      (event.newValue === 'deepseek' || event.newValue === 'openai' || event.newValue === 'compatible')
    ) {
      cookingProvider.set(event.newValue)
    }
    if (event.key === COOKING_API_KEY_KEY) cookingApiKey.set(event.newValue ?? '')
    if (event.key === COOKING_BASE_URL_KEY) cookingBaseUrl.set(event.newValue ?? '')
    if (event.key === COOKING_MODEL_KEY) cookingModel.set(event.newValue ?? '')
    if (
      event.key === COOKING_REASONING_EFFORT_KEY &&
      isCookingReasoningEffort(event.newValue)
    ) {
      cookingReasoningEffort.set(event.newValue)
    }
  })
}

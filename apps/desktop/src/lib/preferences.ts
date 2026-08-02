import { get, writable } from 'svelte/store'

export type Locale = 'zh-CN' | 'en'
export type ThemePreference = 'system' | 'light' | 'dark'
export type NotificationSound = 'chime' | 'soft' | 'alert'

const LOCALE_KEY = 'rambledesk.locale'
const THEME_KEY = 'rambledesk.theme'
const NOTIFICATION_POPUP_KEY = 'rambledesk.notifications.popup'
const NOTIFICATION_SOUND_ENABLED_KEY = 'rambledesk.notifications.sound-enabled'
const NOTIFICATION_SOUND_KEY = 'rambledesk.notifications.sound'
const NOTIFICATION_VOLUME_KEY = 'rambledesk.notifications.volume'
const SPEECH_INPUT_DEVICE_KEY = 'rambledesk.speech.input-device'

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

function initialNotificationSound(): NotificationSound {
  const saved = localStorage.getItem(NOTIFICATION_SOUND_KEY)
  return saved === 'chime' || saved === 'soft' || saved === 'alert' ? saved : 'chime'
}

function initialNotificationVolume() {
  const raw = localStorage.getItem(NOTIFICATION_VOLUME_KEY)
  const saved = raw === null ? Number.NaN : Number(raw)
  return Number.isFinite(saved) && saved >= 0 && saved <= 100 ? saved : 100
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
  })
}

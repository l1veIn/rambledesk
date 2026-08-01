import { get, writable } from 'svelte/store'

export type Locale = 'zh-CN' | 'en'
export type ThemePreference = 'system' | 'light' | 'dark'

const LOCALE_KEY = 'rambledesk.locale'
const THEME_KEY = 'rambledesk.theme'

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

export const locale = writable<Locale>(initialLocale())
export const themePreference = writable<ThemePreference>(initialTheme())

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
  })
}

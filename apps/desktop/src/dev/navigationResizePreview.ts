import '../app.css'
import { mount } from 'svelte'

if (import.meta.env.DEV) {
  // Exercise real persistence across reloads without touching live UI preferences.
  const backing = window.localStorage
  const prefix = 'rambledesk.resize-preview.'
  const keys = () => Array.from({ length: backing.length }, (_, index) => backing.key(index))
    .filter((key): key is string => key !== null && key.startsWith(prefix))
  Object.defineProperty(window, 'localStorage', { configurable: true, value: {
    getItem: (key: string) => backing.getItem(prefix + key),
    setItem: (key: string, value: string) => backing.setItem(prefix + key, String(value)),
    removeItem: (key: string) => backing.removeItem(prefix + key),
    clear: () => keys().forEach(key => backing.removeItem(key)),
    key: (index: number) => keys()[index]?.slice(prefix.length) ?? null,
    get length() { return keys().length },
  } satisfies Storage })
  const params = new URLSearchParams(location.search)
  if (params.has('locale') || !localStorage.getItem('rambledesk.locale')) {
    localStorage.setItem('rambledesk.locale', params.get('locale') === 'zh-CN' ? 'zh-CN' : 'en')
  }
  document.body.classList.add('app-mode')
  const [{ initializePreferences }, { initializeAppearance }, { default: Preview }] = await Promise.all([
    import('$lib/preferences'),
    import('$lib/appearance/appearanceRuntime'),
    import('./NavigationResizePreview.svelte'),
  ])
  initializePreferences()
  const releaseAppearance = initializeAppearance()
  window.addEventListener('pagehide', releaseAppearance, { once: true })
  mount(Preview, { target: document.getElementById('app')! })
}

import '../app.css'
import { mount } from 'svelte'

if (import.meta.env.DEV) {
  const params = new URLSearchParams(location.search)
  const locale = params.get('locale') === 'zh-CN' ? 'zh-CN' : 'en'
  const values = new Map<string, string>([['rambledesk.locale', locale]])
  Object.defineProperty(window, 'localStorage', { value: {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => { values.set(key, value) },
    removeItem: (key: string) => { values.delete(key) },
    clear: () => values.clear(),
    key: (index: number) => [...values.keys()][index] ?? null,
    get length() { return values.size },
  } })
  document.documentElement.lang = locale
  document.documentElement.dataset.theme = 'light'
  const target = document.getElementById('app')!
  if (params.has('settings')) {
    document.body.classList.add('app-mode')
    target.classList.add('appearance-workspace')
    const [{ default: SettingsPanel }, { createOnboardingPreviewCapabilities }, { transport }, { initializePreferences }] = await Promise.all([
      import('$lib/settings/SettingsPanel.svelte'),
      import('./onboardingPreviewCapabilities'),
      import('./agentPreviewFixtures'),
      import('$lib/preferences'),
    ])
    initializePreferences()
    const { initializeAppearance } = await import('$lib/appearance/appearanceRuntime')
    initializeAppearance()
    mount(SettingsPanel, {
      target,
      props: { transport, capabilities: createOnboardingPreviewCapabilities('Windows'), initialSection: params.get('settings') === 'appearance' ? 'appearance' : 'voice' },
    })
  } else {
    const { default: Preview } = await import('./SpeechReviewPreview.svelte')
    mount(Preview, { target })
  }
}

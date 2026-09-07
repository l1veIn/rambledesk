import '../app.css'
import { mount } from 'svelte'

if (import.meta.env.DEV) {
  const values = new Map<string, string>([['rambledesk.locale', 'zh-CN'], ['rambledesk.onboarding.completed', 'true']])
  if (new URLSearchParams(location.search).has('cancelled-agent-restore')) {
    values.set('rambledesk.ui-state', JSON.stringify({ workbench: { workspaceSnapshot: {
      version: 2, views: [{ kind: 'agent-session', sessionId: 'preview' }], activeViewKey: 'agent-session:"preview"',
    } } }))
  }
  Object.defineProperty(window, 'localStorage', { value: {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
    removeItem: (key: string) => values.delete(key),
  } })
  const { default: Preview } = await import('./WorkbenchStartupPreview.svelte')
  mount(Preview, { target: document.getElementById('app')! })
}

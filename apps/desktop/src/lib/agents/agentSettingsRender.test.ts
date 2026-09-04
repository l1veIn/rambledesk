import { describe, expect, it, vi } from 'vitest'
import { render } from 'svelte/server'
import AgentSettings from './AgentSettings.svelte'
import NewManagedSessionForm from './NewManagedSessionForm.svelte'

vi.mock('$lib/preferences', async () => {
  const { writable } = await import('svelte/store')
  return { locale: writable('en') }
})

describe('Agent settings rendering', () => {
  it('redacts the selected configuration environment in new-session errors', () => {
    const { body } = render(NewManagedSessionForm, { props: {
      configs: [{ id: 'config-one', name: 'DeepSeek', host_id: 'dsh', protocol: 'acp', enabled: true,
        command: 'deepseek-acp', args: [], env: { TOKEN: 'private-test-value' }, created_at: 'today', updated_at: 'today' }],
      error: 'Connection failed for private-test-value', onCreate: () => {},
    } })
    expect(body).toContain('Connection failed for [redacted]')
    expect(body).not.toContain('private-test-value')
  })

  it('keeps stored environment values out of settings markup until explicitly revealed', () => {
    const noop = () => {}
    const { body } = render(AgentSettings, { props: {
      configs: [{ id: 'config-one', name: 'DeepSeek', host_id: 'dsh', protocol: 'acp', enabled: true,
        command: 'deepseek-acp', args: [], env: { DEEPSEEK_API_KEY: 'secret-never-rendered' },
        created_at: '2026-09-04', updated_at: '2026-09-04' }],
      onSave: noop, onDelete: noop, onCheck: noop,
    } })
    expect(body).not.toContain('secret-never-rendered')
    expect(body).not.toContain('DEEPSEEK_API_KEY')
    expect(body).toContain('deepseek-acp')
  })
})

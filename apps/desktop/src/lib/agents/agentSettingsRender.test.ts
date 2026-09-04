import { describe, expect, it, vi } from 'vitest'
import { render } from 'svelte/server'
import AgentSettings from './AgentSettings.svelte'

vi.mock('$lib/preferences', async () => {
  const { writable } = await import('svelte/store')
  return { locale: writable('en') }
})

describe('Agent settings rendering', () => {
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

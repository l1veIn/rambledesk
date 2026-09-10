import { describe, expect, it, vi } from 'vitest'
import { render } from 'svelte/server'
import AgentSettings from './AgentSettings.svelte'
import AgentIcon from './AgentIcon.svelte'
import claudeIcon from '../../../../../crates/rambledesk-hosts/assets/icons/claude.svg?raw'
import piIcon from '../../../../../crates/rambledesk-hosts/assets/icons/pi.svg?raw'
import dshIcon from '../../../../../crates/rambledesk-hosts/assets/icons/dsh.svg?raw'
import genericIcon from '../../../../../crates/rambledesk-hosts/assets/icons/generic-terminal.svg?raw'

vi.mock('$lib/preferences', async () => {
  const { writable } = await import('svelte/store')
  return { locale: writable('en') }
})

describe('Agent settings rendering', () => {
  it('reuses adapter logos by backend identity and gives custom backends a generic icon', () => {
    for (const [hostId, icon] of [[' Claude ', claudeIcon], ['pi', piIcon], ['dsh', dshIcon], ['custom-agent', genericIcon], ['constructor', genericIcon]]) {
      const { body } = render(AgentIcon, { props: { hostId } })
      expect(body).toContain(icon)
      expect(body).toContain('aria-hidden="true"')
    }
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

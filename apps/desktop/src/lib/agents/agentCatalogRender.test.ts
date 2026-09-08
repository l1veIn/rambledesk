import { describe, expect, it, vi } from 'vitest'
import { render } from 'svelte/server'
import type { AgentCatalogEntry, AgentConfig, AgentConnectionCheck, AgentInspection } from '$lib/generated/feedback'
import type { AgentCatalogState } from './agentCatalogController'
import { agentLaunchSignature as launchSignature } from './agentDetectionCache'
import AgentCatalog from './AgentCatalog.svelte'

const fixture = vi.hoisted(() => ({ state: undefined as AgentCatalogState | undefined }))
vi.mock('$lib/preferences', async () => ({ locale: (await import('svelte/store')).writable('en') }))
vi.mock('./agentCatalogController', async (original) => ({
  ...await original<typeof import('./agentCatalogController')>(),
  createAgentCatalogController: () => ({ subscribe: (run: (state: AgentCatalogState) => void) => { run(fixture.state!); return () => {} },
    check: vi.fn(),
  }),
}))

const entry: AgentCatalogEntry = {
  id: 'claude-acp', name: 'Claude Code', host_id: 'claude', description: '', connection_kind: 'bridge',
  distribution: { kind: 'npm', package: '@agentclientprotocol/claude-agent-acp', pinned_version: '0.73.0', command: 'claude-agent-acp', node_required: '22.0.0' },
  args: [], dependencies: [], verification: { status: 'unverified', versions: [], note: '' },
}
const profile: AgentConfig = { id: 'claude', catalog_id: entry.id, name: 'Claude work', host_id: 'claude', protocol: 'acp', enabled: true,
  command: '/agents/claude-acp', args: [], env: { API_KEY: 'never-render-this-value' }, created_at: '', updated_at: '' }
const missing: AgentInspection = { agent_id: entry.id, source: 'missing', version: null, command: null, args: [],
  checks: [{ id: 'entry', status: 'fail', message: 'ACP package was not found' }],
  dependencies: [{ command: 'claude', required: false, path: '/usr/local/bin/claude', version: '2.0.0' }] }

function markup(options: { check?: AgentConnectionCheck; inspection?: AgentInspection; profiles?: AgentConfig[]; state?: Partial<AgentCatalogState> } = {}) {
  fixture.state = { entries: [entry], configs: options.profiles ?? (options.check ? [profile] : []), jobs: [],
    inspections: { [entry.id]: options.inspection ?? missing },
    connections: options.check ? { [profile.id]: { signature: launchSignature(profile), result: options.check } } : {},
    checking: [], connecting: [], loading: false, error: '', ...options.state }
  return render(AgentCatalog, { props: { transport: {} as never } }).body
}

describe('Agent detection cards', () => {
  it('offers connection component installation for a detected native CLI without login guidance', () => {
    const body = markup()
    expect(body).toContain('Connect in one click')
    expect(body).toContain('RambleDesk will install its ACP connection component')
    expect(body).not.toContain('data-agent-setup-guide')
    expect(body).not.toContain('Set up Claude Code')
  })

  it('shows only the ACP channel status and hides technical details in advanced settings', () => {
    const body = markup({ check: { ok: true, message: 'ACP connected', details: ['raw handshake detail'] } })
    expect(body).toContain('ACP connection')
    expect(body).not.toContain('Models and conversation')
    const advanced = body.indexOf('data-agent-advanced')
    expect(advanced).toBeGreaterThan(0)
    expect(body.indexOf('raw handshake detail')).toBeGreaterThan(advanced)
    expect(body.indexOf('/agents/claude-acp')).toBeGreaterThan(advanced)
    expect(body).not.toMatch(/<details\b[^>]*\bopen(?:\s|>)/u)
    expect(body).not.toContain('never-render-this-value')
  })

  it('does not offer login for runtime or unknown connection failures', () => {
    for (const options of [
      { inspection: { ...missing, checks: [{ id: 'node', status: 'fail' as const, message: 'Node.js is missing' }] } },
      { check: { ok: false, message: 'Unknown failure', details: [] } },
    ]) {
      const body = markup(options)
      expect(body).not.toContain('data-agent-setup-guide')
      expect(body).not.toContain('Run in a terminal:')
    }
  })
})

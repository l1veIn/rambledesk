import { get } from 'svelte/store'
import { describe, expect, it } from 'vitest'
import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
import type { AgentCatalogEntry, AgentConfig, AgentConnectionCheck, AgentInspection } from '$lib/generated/feedback'
import { agentLaunchSignature } from './agentDetectionCache'
import { catalogConfiguration, createAgentCatalogController, sortedAgentListItems } from './agentCatalogController'

const entry: AgentCatalogEntry = {
  id: 'deepseek-acp', name: 'DeepSeek (DSH)', host_id: 'dsh', description: '', connection_kind: 'bridge',
  distribution: { kind: 'npm', package: 'deepseek-acp', pinned_version: '0.8.0', command: 'deepseek-acp', node_required: '22.0.0' },
  args: [], dependencies: [], verification: { status: 'unverified', versions: [], note: '' },
}
const inspection: AgentInspection = { agent_id: entry.id, source: 'managed', version: '0.8.0', command: '/node', args: ['/agents/deepseek-acp/index.js'], dependencies: [], checks: [] }
const config: AgentConfig = { ...catalogConfiguration(entry, inspection), id: 'saved', created_at: '', updated_at: '' }
const connected: AgentConnectionCheck = { ok: true, message: 'ACP connected', details: [] }
const missing: AgentInspection = { ...inspection, source: 'missing', command: null, checks: [{ id: 'entry', status: 'fail', message: 'Not found' }] }
async function flush() { for (let index = 0; index < 40; index++) await Promise.resolve() }
function harness() {
  const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
    .resolve('listAvailableAgents', [entry]).resolve('listAgentConfigs', [config]).resolve('listAgentInstallJobs', [])
  return { transport, controller: createAgentCatalogController(transport) }
}

describe('Agent catalog presentation', () => {
  it('keeps the scan busy across refresh, installation inspection and ACP handshake', async () => {
    const { controller, transport } = harness()
    controller.start(); await controller.refresh()
    let finishRefresh!: (entries: AgentCatalogEntry[]) => void
    let finishInspection!: (result: AgentInspection) => void
    let finishConnection!: (result: AgentConnectionCheck) => void
    transport.handle('listAvailableAgents', () => new Promise(resolve => { finishRefresh = resolve }))
      .handle('inspectAgentInstallation', () => new Promise(resolve => { finishInspection = resolve }))
      .handle('checkAgentConfig', () => new Promise(resolve => { finishConnection = resolve }))
    const scan = controller.detectAll()
    expect(get(controller).detecting).toBe(true)
    expect(controller.detectAll()).toBe(scan)
    await flush()
    expect(get(controller).checking).toEqual([])
    finishRefresh([entry]); await flush()
    expect(get(controller).detecting).toBe(true)
    expect(get(controller).checking).toContain(entry.id)
    finishInspection(inspection); await flush()
    expect(get(controller).detecting).toBe(true)
    expect(get(controller).connecting).toContain(`config:${config.id}`)
    finishConnection(connected); await scan
    expect(get(controller).detecting).toBe(false)
    controller.dispose()
  })

  it('sorts by current connection status and preserves catalog order within each status', () => {
    const ids = ['unchecked', 'connected-b', 'attention', 'prepare', 'missing', 'checking', 'connected-a', 'stale']
    const entries = ids.map(id => ({ ...entry, id, connection_kind: id === 'missing' ? 'native' as const : 'bridge' as const }))
    const configs = ['connected-b', 'connected-a', 'attention', 'stale', 'legacy'].map(id => ({ ...config, id, catalog_id: id }))
    const { controller } = harness()
    const state = { ...get(controller), entries, configs, checking: ['checking'],
      inspections: { prepare: { ...missing, agent_id: 'prepare' }, missing: { ...missing, agent_id: 'missing' } },
      connections: Object.fromEntries(configs.map(profile => [profile.id, {
        signature: profile.id === 'stale' ? 'old-launch' : agentLaunchSignature(profile),
        result: profile.id === 'attention' ? { ok: false, message: 'Failed', details: [] } : connected,
      }])),
    }
    expect(sortedAgentListItems(state).map(row => row.entry?.id ?? row.config?.id)).toEqual([
      'connected-b', 'connected-a', 'checking', 'attention', 'prepare', 'unchecked', 'stale', 'missing', 'legacy',
    ])
    expect(state.entries.map(entry => entry.id)).toEqual(ids)
  })
})

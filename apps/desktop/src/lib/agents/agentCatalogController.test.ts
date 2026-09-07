import { get } from 'svelte/store'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import { readAgentDetectionCache } from './agentDetectionCache'
import { configureClientDiagnostics, type ClientDiagnosticEvent } from '$lib/diagnostics/clientDiagnostics'
import type { AgentCatalogEntry, AgentConfig, AgentConnectionCheck, AgentInspection, AgentInstallJob } from '$lib/generated/feedback'
import { agentConnectionResult, agentListItems, agentStatus, catalogConfiguration, configurationsForAgent, connectionPreparationAvailable, createAgentCatalogController, manualAgentConfiguration } from './agentCatalogController'

const entry: AgentCatalogEntry = {
  id: 'deepseek-acp', name: 'DeepSeek ACP', host_id: 'dsh', description: '', connection_kind: 'bridge',
  distribution: { kind: 'npm', package: 'deepseek-acp', pinned_version: '0.8.0', command: 'deepseek-acp', node_required: '22.0.0' },
  args: [], dependencies: [], verification: { status: 'unverified', versions: [], note: '' },
}
const inspection: AgentInspection = { agent_id: entry.id, source: 'managed', version: '0.8.0', command: 'C:/node.exe', args: ['C:/agents/new/node_modules/deepseek-acp/index.js'], dependencies: [], checks: [] }
const job: AgentInstallJob = { id: 'job', agent_id: entry.id, phase: 'installing', messages: [], result: null, cancel_requested: false }
const config: AgentConfig = { ...catalogConfiguration(entry, inspection), id: 'saved', created_at: '', updated_at: '' }
const connected: AgentConnectionCheck = { ok: true, message: 'ACP connected', details: ['Handshake only'] }
const missing: AgentInspection = { ...inspection, source: 'missing', command: null, checks: [{ id: 'entry', status: 'fail', message: 'Not found' }] }
async function flush() { for (let index = 0; index < 40; index++) await Promise.resolve() }
function harness(options: { jobs?: AgentInstallJob[]; configs?: AgentConfig[]; entries?: AgentCatalogEntry[]; inspection?: AgentInspection } = {}) {
  let configs = [...options.configs ?? []]
  const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
    .resolve('listAvailableAgents', options.entries ?? [entry]).resolve('listAgentInstallJobs', options.jobs ?? [])
    .handle('inspectAgentInstallation', ({ agent_id }) => ({ ...options.inspection ?? inspection, agent_id }))
    .handle('listAgentConfigs', () => configs)
    .handle('resolveCatalogAgent', ({ agent_id }) => {
      const saved = { ...config, id: `resolved:${agent_id}`, catalog_id: agent_id }
      configs = [...configs, saved]
      return saved
    })
    .handle('saveAgentConfig', input => {
      const saved = { ...input, id: input.id ?? 'new', created_at: '', updated_at: '' }
      configs = [...configs.filter(config => config.id !== saved.id), saved]
      return saved
    })
    .handle('deleteAgentConfig', ({ agent_config_id }) => { configs = configs.filter(config => config.id !== agent_config_id) })
    .resolve('checkAgentConfig', connected)
  const controller = createAgentCatalogController(transport)
  const changed = () => transport.emit(APPLICATION_EVENTS_STREAM, { type: 'invalidate', runtime_generation: 'test', revision: '1', resources: [{ kind: 'agent_configurations' }] })
  return { transport, controller, changed }
}
let stopDiagnostics: (() => void) | undefined
function captureDiagnostics() {
  const events: ClientDiagnosticEvent[] = []
  stopDiagnostics = configureClientDiagnostics(event => { events.push(event) })
  return events
}
afterEach(() => { vi.useRealTimers(); stopDiagnostics?.(); stopDiagnostics = undefined })

describe('Simplified agent management', () => {
  it('records explicit scans and cache reuse without logging configurations or repeated cheap invalidations', async () => {
    const events = captureDiagnostics()
    const secret = 'private-agent-canary'
    const { controller, transport, changed } = harness({ configs: [{ ...config, name: secret, command: `C:/${secret}/node.exe`, env: { API_KEY: secret } }] })
    controller.start(); await controller.detectAll('onboarding')
    const scan = events.find(event => event.activity === 'agent_detection' && event.details?.action === 'scan' && event.outcome === 'ok')
    expect(scan).toMatchObject({ details: { reason: 'onboarding', rescan: true, entry_count: 1, checked_count: 1, failed_count: 0 }, durationMs: expect.any(Number) })
    expect(events.find(event => event.operationId === scan?.operationId && event.outcome === 'started')).toBeDefined()
    expect(events.find(event => event.activity === 'agent_connection' && event.outcome === 'ok')).toMatchObject({ details: { agent: 'deepseek-acp', agent_kind: 'bridge' } })
    await controller.refresh()
    expect(events.filter(event => event.activity === 'agent_catalog_refresh').at(-1)).toMatchObject({ outcome: 'ok', details: { cache_hit: true, rescan: false } })
    changed(); await flush()
    const count = events.length
    for (let index = 0; index < 5; index++) { changed(); await flush() }
    expect(events).toHaveLength(count)
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(1)
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(1)
    expect(JSON.stringify(events)).not.toContain(secret)
    controller.dispose()
  })

  it('maps unknown catalog identifiers to custom before recording detection and connection diagnostics', async () => {
    const events = captureDiagnostics()
    const canary = 'private-custom-catalog-canary'
    const { controller } = harness({ entries: [{ ...entry, id: canary }], configs: [{ ...config, catalog_id: canary }], inspection: { ...inspection, agent_id: canary } })
    controller.start(); await controller.detectAll()
    const operations = events.filter(event => event.details?.agent_kind)
    expect(operations.length).toBeGreaterThan(0)
    expect(operations.every(event => event.details?.agent === 'custom')).toBe(true)
    expect(JSON.stringify(events)).not.toContain(canary)
    controller.dispose()
  })

  it('retains partial scan failures in diagnostics after later probes succeed and logs only one post-install target', async () => {
    const events = captureDiagnostics()
    const { controller, transport } = harness({ entries: [entry, { ...entry, id: 'second' }] })
    transport.handle('inspectAgentInstallation', ({ agent_id }) => {
      if (agent_id === entry.id) throw new Error('secret-diagnostic-error C:/private/config')
      return { ...inspection, agent_id }
    })
    controller.start(); await controller.detectAll()
    expect(events.find(event => event.details?.action === 'scan' && event.outcome === 'failed')).toMatchObject({ details: { failed_count: 1 } })
    expect(JSON.stringify(events)).not.toContain('secret-diagnostic-error')
    transport.resolve('inspectAgentInstallation', inspection).resolve('installAgent', { ...job, phase: 'complete' })
    const before = transport.callsFor('inspectAgentInstallation').length
    await controller.install(entry.id)
    expect(transport.callsFor('inspectAgentInstallation').slice(before).map(call => call.input.agent_id)).toEqual([entry.id])
    expect(events.find(event => event.activity === 'agent_install' && event.details?.action === 'install' && event.outcome === 'ok')).toMatchObject({ details: { status: 'complete' } })
    expect(events.filter(event => event.details?.action === 'recheck').map(event => event.outcome)).toEqual(['started', 'ok'])
    expect(events.find(event => event.details?.action === 'recheck' && event.outcome === 'ok')).toMatchObject({ details: { target_count: 1, checked_count: 1, reason: 'post_install' } })
    controller.dispose()
  })

  it('records cancelled install jobs without inspecting or checking any agent', async () => {
    const events = captureDiagnostics()
    const { controller, transport } = harness()
    transport.resolve('installAgent', { ...job, phase: 'cancelled' })
    controller.start(); await controller.refresh(); await controller.install(entry.id)
    expect(events.find(event => event.activity === 'agent_install' && event.outcome === 'cancelled')).toMatchObject({ details: { action: 'install', status: 'cancelled' } })
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(0)
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(0)
    controller.dispose()
  })
  it('cheap-loads settings on mount and invalidation without probing or launching Agents', async () => {
    const { controller, transport, changed } = harness({ configs: [config], jobs: [{ ...job, phase: 'complete' }] })
    controller.start(); await controller.refresh()
    changed(); await flush(); await controller.refresh()
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(0)
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(0)
    expect(transport.callsFor('resolveCatalogAgent')).toHaveLength(0)
    expect(agentStatus(agentListItems(get(controller).entries, get(controller).configs)[0], get(controller))).toBe('unchecked')
    controller.dispose()
  })

  it('reuses explicit detection and handshake results when settings reopen', async () => {
    const { controller, transport } = harness()
    controller.start(); await controller.detectAll(); controller.dispose()
    const reopened = createAgentCatalogController(transport)
    reopened.start(); await reopened.refresh()
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(1)
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(1)
    const state = get(reopened)
    expect(agentStatus(agentListItems(state.entries, state.configs)[0], state)).toBe('connected')
    expect(readAgentDetectionCache(transport).inspections[entry.id]).toEqual(inspection)
    reopened.dispose()
  })

  it('publishes a completed handshake to settings reopened while the check was running', async () => {
    const { controller, transport } = harness({ configs: [config] })
    let finish!: (result: AgentConnectionCheck) => void
    transport.handle('checkAgentConfig', () => new Promise(resolve => { finish = resolve }))
    controller.start(); await controller.refresh()
    const checking = controller.check(config.id)
    await flush(); controller.dispose()
    const reopened = createAgentCatalogController(transport)
    reopened.start(); await reopened.refresh()
    finish(connected); await checking
    expect(agentConnectionResult(config, get(reopened))).toEqual(connected)
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(1)
    reopened.dispose()
  })

  it('retains installation evidence completed after leaving settings', async () => {
    const { controller, transport } = harness()
    let finish!: (result: AgentInspection) => void
    transport.handle('inspectAgentInstallation', () => new Promise(resolve => { finish = resolve }))
    controller.start(); await controller.refresh()
    const inspecting = controller.inspect(entry.id)
    controller.dispose(); finish(missing); await inspecting
    const reopened = createAgentCatalogController(transport)
    reopened.start(); await reopened.refresh()
    expect(get(reopened).inspections[entry.id]).toEqual(missing)
    expect(get(controller).inspections).toEqual({})
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(1)
    reopened.dispose()
  })

  it('keeps a newer explicit check when a closed settings page finishes its older check', async () => {
    const { controller, transport } = harness({ configs: [config] })
    let finish!: (result: AgentConnectionCheck) => void
    transport.handle('checkAgentConfig', () => new Promise(resolve => { finish = resolve }))
    controller.start(); await controller.refresh()
    const olderCheck = controller.check(config.id)
    await flush(); controller.dispose()
    const reopened = createAgentCatalogController(transport)
    reopened.start(); await reopened.refresh()
    const failed = { ok: false, message: 'Authentication expired', details: [] }
    transport.resolve('checkAgentConfig', failed)
    await reopened.check(config.id)
    finish(connected); await olderCheck
    expect(agentConnectionResult(config, get(reopened))).toEqual(failed)
    expect(readAgentDetectionCache(transport).connections[config.id].result).toEqual(failed)
    reopened.dispose()
  })

  it('drops old runtime evidence immediately and ignores probes finishing after replacement', async () => {
    const { controller, transport } = harness({ configs: [config] })
    controller.start(); await controller.detectAll()
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'ready', runtime_generation: 'first', revision: '1' })
    await flush()
    let finishInspection!: (result: AgentInspection) => void
    let finishConnection!: (result: AgentConnectionCheck) => void
    transport.handle('inspectAgentInstallation', () => new Promise(resolve => { finishInspection = resolve }))
    transport.handle('checkAgentConfig', () => new Promise(resolve => { finishConnection = resolve }))
    const inspecting = controller.inspect(entry.id)
    const checking = controller.check(config.id)
    await flush()
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'invalidate', runtime_generation: 'replacement', revision: '1', resources: [{ kind: 'navigation' }] })
    expect(get(controller).connections).toEqual({})
    expect(get(controller).inspections).toEqual({})
    finishInspection(inspection); finishConnection(connected)
    await Promise.all([inspecting, checking])
    expect(readAgentDetectionCache(transport)).toEqual({ inspections: {}, connections: {} })
    expect(get(controller).connections).toEqual({})
    expect(get(controller).inspections).toEqual({})
    controller.dispose()
  })

  it('evicts a changed launch so restoring old settings still requires another check', async () => {
    const { controller, transport } = harness({ configs: [config] })
    controller.start(); await controller.detectAll()
    await controller.save({ ...config, env: { TOKEN: 'changed' } })
    expect(readAgentDetectionCache(transport).connections).toEqual({})
    await controller.save(config)
    expect(agentConnectionResult(config, get(controller))).toBeUndefined()
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(1)
    controller.dispose()
  })

  it('clears cached results on runtime changes without launching a fresh scan', async () => {
    const { controller, transport } = harness()
    controller.start(); await controller.detectAll()
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'invalidate', runtime_generation: 'first', revision: '1', resources: [{ kind: 'agent_configurations' }] })
    await flush()
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'invalidate', runtime_generation: 'replacement', revision: '1', resources: [{ kind: 'agent_configurations' }] })
    await flush()
    expect(get(controller).inspections).toEqual({})
    expect(get(controller).connections).toEqual({})
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(1)
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(1)
    const unrelated = harness()
    unrelated.controller.start(); await unrelated.controller.refresh()
    expect(get(unrelated.controller).inspections).toEqual({})
    controller.dispose(); unrelated.controller.dispose()
  })

  it('checks only the explicitly installed Agent when installation completes immediately', async () => {
    const { controller, transport } = harness({ entries: [entry, { ...entry, id: 'unrelated' }] })
    controller.start(); await controller.refresh()
    transport.resolve('installAgent', { ...job, phase: 'complete' })
    await controller.install(entry.id)
    expect(transport.callsFor('inspectAgentInstallation').map(call => call.input.agent_id)).toEqual([entry.id])
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(1)
    controller.dispose()
  })
  it('groups known agents while retaining every legacy and custom configuration', () => {
    const other = { ...config, catalog_id: 'dsh', id: 'dsh' }
    const custom = { ...config, id: 'custom', catalog_id: undefined }
    const edited = { ...config, id: 'edited', name: 'My account', command: 'custom-wrapper', args: ['anything'] }
    expect(configurationsForAgent(entry, [config, other, custom, edited])).toEqual([config, edited])
    const rows = agentListItems([entry], [config, other, custom, edited])
    expect(rows.map(item => [item.key, item.name])).toEqual([
      ['catalog:deepseek-acp', entry.name], ['config:dsh', entry.name], ['config:custom', entry.name],
    ])
    expect(rows[0].configs).toEqual([config, edited])
    expect(catalogConfiguration(entry, { ...inspection, env: { DEFAULT: 'retain' } }).env).toEqual({ DEFAULT: 'retain' })
    expect(() => catalogConfiguration(entry, { ...inspection, checks: [{ id: 'node', status: 'fail', message: 'Missing' }] })).toThrow('failed checks')
    expect(() => catalogConfiguration(entry, missing)).toThrow('Install')
  })

  it('keeps missing programs, missing connection components and runtime failures distinct', async () => {
    const native = { ...entry, id: 'native', connection_kind: 'native' as const }
    const { controller } = harness({ entries: [entry, native], inspection: missing })
    controller.start(); await controller.detectAll()
    const state = get(controller)
    const rows = agentListItems(state.entries, state.configs)
    expect(agentStatus(rows[0], state)).toBe('prepare')
    expect(agentStatus(rows[1], state)).toBe('missing')
    const noNode = { ...missing, checks: [...missing.checks, { id: 'node', status: 'fail' as const, message: 'Node missing' }] }
    expect(agentStatus(rows[0], { ...state, inspections: { [entry.id]: noNode } })).toBe('attention')
    expect(connectionPreparationAvailable(entry, missing)).toBe(true)
    expect(connectionPreparationAvailable(entry, noNode)).toBe(false)
    expect(connectionPreparationAvailable(native, missing)).toBe(false)
    expect(agentStatus(rows[1], { ...state, inspections: { [native.id]: noNode } })).toBe('missing')
    expect(agentStatus(rows[1], { ...state, inspections: { [native.id]: { ...noNode, source: 'system' } } })).toBe('attention')
    expect(agentStatus(rows[0], { ...state, inspections: { [entry.id]: { ...noNode, source: 'managed' } } })).toBe('attention')
    expect(connectionPreparationAvailable(entry, { ...inspection, checks: [{ id: 'npm', status: 'warn', message: 'npm unavailable' }] })).toBe(false)
    controller.dispose()
  })

  it('offers connection preparation when the missing Pi dependency is bundled by the installer', async () => {
    const pi = { ...entry, id: 'pi-acp', dependencies: [{ command: 'pi', required: true, package: '@mariozechner/pi-coding-agent', pinned_version: '1.0.0', instructions: 'Pi required' }] }
    const missingPi = { ...inspection, dependencies: [{ command: 'pi', required: true, path: null, version: null }], checks: [{ id: 'dependency_pi', status: 'fail' as const, message: 'Pi required' }] }
    const { controller } = harness({ entries: [pi], inspection: missingPi })
    controller.start(); await controller.detectAll()
    const state = get(controller)
    expect(connectionPreparationAvailable(pi, missingPi)).toBe(true)
    expect(agentStatus(agentListItems(state.entries, state.configs)[0], state)).toBe('prepare')
    expect(connectionPreparationAvailable({ ...pi, dependencies: [{ ...pi.dependencies[0], package: null }] }, missingPi)).toBe(false)
    controller.dispose()
  })

  it('explicitly detects agents and checks the ACP handshake only once until the next request', async () => {
    const { controller, transport, changed } = harness()
    controller.start(); await controller.detectAll()
    expect(transport.callsFor('resolveCatalogAgent')).toHaveLength(1)
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(1)
    changed(); await flush()
    await controller.refresh()
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(1)
    const state = get(controller)
    expect(agentStatus(agentListItems(state.entries, state.configs)[0], state)).toBe('connected')
    expect(transport.callsFor('sendManagedPrompt')).toHaveLength(0)
    await controller.detectAll()
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(2)
    controller.dispose()
  })

  it('checks the saved actual command even when catalog detection misses it and redacts failures', async () => {
    const saved = { ...config, command: 'D:/Private/agent.exe', env: { TOKEN: 'secret-value' } }
    const { controller, transport } = harness({ configs: [saved], inspection: missing })
    transport.reject('checkAgentConfig', new Error('Cannot connect with secret-value'))
    controller.start(); await controller.detectAll()
    expect(transport.callsFor('resolveCatalogAgent')).toHaveLength(0)
    expect(transport.callsFor('checkAgentConfig')[0].input).toEqual({ agent_config_id: saved.id })
    const state = get(controller)
    expect(agentConnectionResult(saved, state)).toEqual({ ok: false, message: 'Cannot connect with [redacted]', details: [] })
    expect(agentStatus(agentListItems(state.entries, state.configs)[0], state)).toBe('attention')
    expect(transport.callsFor('saveAgentConfig')).toHaveLength(0)
    controller.dispose()
  })

  it('deduplicates the same handshake when resolution emits a configuration event', async () => {
    const { controller, transport, changed } = harness()
    transport.handle('resolveCatalogAgent', () => {
      transport.resolve('listAgentConfigs', [config])
      changed()
      return config
    })
    let finish!: (result: AgentConnectionCheck) => void
    transport.handle('checkAgentConfig', () => new Promise(resolve => { finish = resolve }))
    controller.start(); await flush()
    void controller.detectAll(); await flush()
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(1)
    finish(connected); await flush()
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(1)
    controller.dispose()
  })

  it('keeps disabled profiles disabled across refresh and historical installation completion', async () => {
    const disabled = { ...config, enabled: false, env: { TOKEN: 'preserve' } }
    const { controller, transport } = harness({ configs: [disabled], jobs: [{ ...job, phase: 'complete' }] })
    controller.start(); await controller.refresh(); await controller.refresh()
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(0)
    expect(transport.callsFor('saveAgentConfig')).toHaveLength(0)
    await controller.check(disabled.id)
    expect(transport.callsFor('saveAgentConfig')[0].input).toMatchObject({ id: disabled.id, enabled: true, env: disabled.env })
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(1)
    controller.dispose()
  })

  it('limits concurrent handshakes and skips queued profiles deleted before launch', async () => {
    const configs = Array.from({ length: 5 }, (_, index) => ({ ...config, id: String(index), catalog_id: undefined }))
    const { controller, transport } = harness({ configs, entries: [] })
    const finish: Array<(result: AgentConnectionCheck) => void> = []
    transport.handle('checkAgentConfig', () => new Promise(resolve => finish.push(resolve)))
    controller.start(); await flush()
    void controller.detectAll(); await flush()
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(2)
    await controller.remove('2')
    finish.shift()!(connected); await flush()
    expect(transport.callsFor('checkAgentConfig').map(call => call.input.agent_config_id)).toEqual(['0', '1', '3'])
    controller.dispose()
    while (finish.length) finish.shift()!(connected)
    await flush()
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(3)
  })

  it('never retries failed catalog resolution in response to unrelated configuration events', async () => {
    const { controller, transport, changed } = harness()
    transport.reject('resolveCatalogAgent', new Error('Could not resolve'))
    controller.start(); await controller.detectAll()
    changed(); await flush(); changed(); await flush()
    expect(transport.callsFor('resolveCatalogAgent')).toHaveLength(1)
    await controller.detectAll()
    expect(transport.callsFor('resolveCatalogAgent')).toHaveLength(2)
    controller.dispose()
  })

  it('marks an edited launch unchecked without automatically starting another handshake', async () => {
    const { controller, transport } = harness({ configs: [config] })
    let finish!: (result: AgentConnectionCheck) => void
    transport.handle('checkAgentConfig', () => new Promise(resolve => { finish = resolve }))
    controller.start(); await flush()
    void controller.check(config.id); await flush()
    const updated = await controller.save({ ...config, command: 'D:/new/agent.exe', env: { TOKEN: 'retain' } })
    expect(agentConnectionResult(updated, get(controller))).toBeUndefined()
    finish(connected)
    await flush()
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(1)
    expect(agentConnectionResult(updated, get(controller))).toBeUndefined()
    void controller.check(config.id); await flush()
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(2)
    finish(connected); await flush()
    expect(agentConnectionResult(updated, get(controller))?.ok).toBe(true)
    controller.dispose()
  })

  it('replaces a known launch path while preserving identity, credentials and account environment', () => {
    const saved = { ...config, env: { TOKEN: 'retain', HOME: '/custom-home' }, name: 'Existing account' }
    const native = { ...entry, args: ['--profile', 'acp'] }
    expect(manualAgentConfiguration(native, ' "D:/agents/my agent.exe" ', saved)).toEqual({
      id: saved.id, catalog_id: entry.id, name: saved.name, host_id: saved.host_id, protocol: 'acp', enabled: true,
      command: 'D:/agents/my agent.exe', args: ['--profile', 'acp'], env: saved.env,
    })
    expect(manualAgentConfiguration(native, '/opt/my agent/bin/agent').command).toBe('/opt/my agent/bin/agent')
    expect(() => manualAgentConfiguration(native, 'agent')).toThrow('full path')
    expect(() => manualAgentConfiguration(native, 'D:/agent.js')).toThrow('runtime')
    expect(() => manualAgentConfiguration(native, 'D:/agent.exe\n--unsafe')).toThrow('full path')
  })

  it('checks once after an installation explicitly requested in this view completes', async () => {
    vi.useFakeTimers()
    const { transport, controller } = harness({ jobs: [job] })
    controller.start(); await flush()
    transport.resolve('installAgent', job)
    await controller.install(entry.id)
    expect(get(controller).jobs[0].phase).toBe('installing')
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(0)
    transport.resolve('listAgentInstallJobs', [{ ...job, phase: 'complete' }])
    await vi.advanceTimersByTimeAsync(500)
    expect(get(controller).jobs[0].phase).toBe('complete')
    expect(get(controller).inspections[entry.id]).toEqual(inspection)
    await vi.advanceTimersByTimeAsync(1000)
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(1)
    expect(transport.callsFor('listAgentInstallJobs')).toHaveLength(3)
    controller.dispose()
  })

  it('stops polling on disposal without cancelling an owned installation', async () => {
    vi.useFakeTimers()
    const { transport, controller } = harness({ jobs: [job] })
    controller.start(); await flush(); controller.dispose()
    await vi.advanceTimersByTimeAsync(2000)
    await controller.install(entry.id); await controller.cancel(job.id)
    expect(transport.callsFor('listAgentInstallJobs')).toHaveLength(1)
    expect(transport.callsFor('cancelAgentInstall')).toHaveLength(0)
    expect(transport.callsFor('installAgent')).toHaveLength(0)
  })

  it('deduplicates discovery probes and ignores responses after disposal', async () => {
    const { transport, controller } = harness()
    let finish!: (value: AgentInspection) => void
    transport.handle('inspectAgentInstallation', () => new Promise(resolve => { finish = resolve }))
    controller.start(); await flush()
    const first = controller.inspect(entry.id)
    const second = controller.inspect(entry.id)
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(1)
    controller.dispose(); finish(inspection)
    await Promise.all([first, second])
    expect(get(controller).inspections).toEqual({})
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(0)
  })

  it('retains cancellation progress until terminal cleanup', async () => {
    vi.useFakeTimers()
    const { transport, controller } = harness({ jobs: [job] })
    controller.start(); await flush()
    transport.resolve('cancelAgentInstall', undefined).resolve('listAgentInstallJobs', [{ ...job, cancel_requested: true }])
    await controller.cancel(job.id)
    expect(get(controller).jobs[0]).toMatchObject({ phase: 'installing', cancel_requested: true })
    transport.resolve('listAgentInstallJobs', [{ ...job, phase: 'cancelled', cancel_requested: true }])
    await vi.advanceTimersByTimeAsync(500)
    expect(get(controller).jobs[0].phase).toBe('cancelled')
    controller.dispose()
  })
})

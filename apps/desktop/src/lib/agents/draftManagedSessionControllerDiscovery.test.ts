import { get } from 'svelte/store'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import type { AgentCatalogEntry, AgentInspection, AgentInstallJob } from '$lib/generated/feedback'
import {
  agentNeedsPreparation,
  canPrepareAgentConnection,
  createDraftManagedSessionController,
  draftAgentChoices,
} from './draftManagedSessionController'
import { readAgentDetectionCache, rememberAgentInspection } from './agentDetectionCache'
import {
  catalog,
  config,
  deferred,
  flush,
  inspection,
  releaseDiagnostics,
  setup,
  snapshot,
} from './draftManagedSessionControllerTestHarness'

afterEach(() => {
  releaseDiagnostics()
})

describe('managed draft lifecycle: discovery and catalog choices', () => {
  it('uses cached discovery on mount, remount, focus-style refresh and configuration invalidation without probing', async () => {
    const { controller, transport } = setup()
    transport.resolve('listAvailableAgents', [catalog])
    rememberAgentInspection(transport, inspection)
    controller.start(); await flush()
    expect(get(controller).phase).toBe('ready')
    expect(get(controller).choices.some(choice => choice.key === 'catalog:pi')).toBe(true)
    controller.start(); await controller.refreshChoices(false)
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'invalidate', runtime_generation: 'runtime', revision: '1', resources: [{ kind: 'agent_configurations' }] })
    await flush()
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(0)
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(0)
    expect(transport.callsFor('prepareManagedSession')).toHaveLength(1)
    await controller.close()
  })

  it('makes saved profiles usable even while the catalog list is still loading', async () => {
    const { controller, transport } = setup()
    const listing = deferred<AgentCatalogEntry[]>()
    transport.handle('listAvailableAgents', () => listing.promise)
    controller.start(); await flush()
    expect(get(controller)).toMatchObject({ phase: 'ready', loadingChoices: false, choice: 'config:config' })
    await controller.close()
    listing.resolve([catalog]); await flush()
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(0)
  })

  it('connects configured agents before slow probes finish and bounds repeated scans to three probes', async () => {
    const { controller, transport } = setup()
    const entries = Array.from({ length: 6 }, (_, index) => ({ ...catalog, id: `catalog-${index}` }))
    const pending = new Map(entries.map(entry => [entry.id, deferred<AgentInspection>()]))
    transport.resolve('listAvailableAgents', entries).handle('inspectAgentInstallation', ({ agent_id }) => pending.get(agent_id)!.promise)
    controller.start(); await flush()
    expect(get(controller)).toMatchObject({ phase: 'ready', loadingChoices: false, choice: 'config:config' })
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(0)
    const scan = controller.refreshChoices(true); await flush()
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(3)
    controller.start(); await flush()
    expect(controller.refreshChoices(true)).toBe(scan)
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(3)
    pending.get('catalog-0')!.resolve({ ...inspection, agent_id: 'catalog-0' }); await flush()
    expect(get(controller).choices.map(choice => choice.key)).toContain('catalog:catalog-0')
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(4)
    await controller.close()
    for (const [id, result] of pending) result.resolve({ ...inspection, agent_id: id })
    await scan
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(4)
    expect(get(controller).choices.map(choice => choice.key)).not.toContain('catalog:catalog-1')
  })

  it('leaves configured agents ready when discovery fails and reuses completed discovery on tab remount', async () => {
    const { controller, transport } = setup()
    transport.resolve('listAvailableAgents', [catalog]).handle('inspectAgentInstallation', () => { throw new Error('Probe failed') })
    controller.start(); await flush()
    await controller.refreshChoices(true)
    expect(get(controller)).toMatchObject({ phase: 'ready', loadingChoices: false, choicesError: 'Some installed agents could not be checked.' })
    transport.resolve('inspectAgentInstallation', inspection)
    await controller.refreshChoices(true)
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(2)
    controller.start(); await flush()
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(2)
    await controller.refreshChoices(true)
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(3)
    await controller.close()
  })

  it('retains an explicit discovery result that finishes after closing the draft', async () => {
    const { controller, transport } = setup()
    const probe = deferred<AgentInspection>()
    transport.resolve('listAvailableAgents', [catalog]).handle('inspectAgentInstallation', () => probe.promise)
    controller.start(); await flush()
    const scan = controller.refreshChoices(true); await flush()
    await controller.close()
    probe.resolve(inspection); await scan
    expect(readAgentDetectionCache(transport).inspections.pi).toEqual(inspection)
  })

  it('does not cache a draft discovery response from the replaced runtime', async () => {
    const { controller, transport } = setup()
    const probe = deferred<AgentInspection>()
    transport.resolve('listAvailableAgents', [catalog]).handle('inspectAgentInstallation', () => probe.promise)
    controller.start(); await flush()
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'ready', runtime_generation: 'first', revision: '1' })
    await flush()
    const scan = controller.refreshChoices(true); await flush()
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'ready', runtime_generation: 'replacement', revision: '1' })
    probe.resolve(inspection); await scan
    expect(readAgentDetectionCache(transport).inspections).toEqual({})
    expect(get(controller).choices.find(choice => choice.catalogId === 'pi')?.inspection).toBeUndefined()
    await controller.close()
  })

  it('never restores the prior agent or directory when a discovery result arrives after a user selection', async () => {
    const { controller, transport } = setup()
    const probe = deferred<AgentInspection>()
    const alternate = { ...config, id: 'alternate', name: 'Another agent' }
    transport.resolve('listAgentConfigs', [config, alternate]).resolve('listAvailableAgents', [catalog])
      .handle('inspectAgentInstallation', () => probe.promise)
      .handle('prepareManagedSession', ({ agent_config_id }) => snapshot(agent_config_id))
    controller.start(); await flush()
    void controller.refreshChoices(true); await flush()
    controller.select('config:alternate', '/another'); await flush()
    expect(get(controller)).toMatchObject({ choice: 'config:alternate', cwd: '/another', phase: 'ready', snapshot: { session: { session_id: 'alternate' } } })
    probe.resolve(inspection); await flush()
    expect(get(controller)).toMatchObject({ choice: 'config:alternate', cwd: '/another', phase: 'ready', snapshot: { session: { session_id: 'alternate' } } })
    expect(transport.callsFor('prepareManagedSession').map(call => call.input)).toEqual([
      { agent_config_id: 'config', cwd: '/repo' }, { agent_config_id: 'alternate', cwd: '/another' },
    ])
    await controller.close()
  })

  it('keeps custom profiles distinct and matches catalog identity without launch heuristics', () => {
    const custom = { ...config, id: 'custom', enabled: false }
    const linked = { ...config, id: 'linked', catalog_id: 'pi', name: 'My Pi' }
    expect(draftAgentChoices([custom], [catalog], [inspection]).map((choice) => choice.key)).toEqual(['config:custom', 'catalog:pi'])
    expect(draftAgentChoices([custom, linked], [catalog], [inspection]).map((choice) => choice.key)).toEqual(['config:custom', 'config:linked'])
    const missing = draftAgentChoices([], [catalog], [{ ...inspection, source: 'missing', command: null }])
    expect(missing.map(choice => choice.key)).toEqual(['catalog:pi'])
    expect(agentNeedsPreparation(missing[0])).toBe(true)
    expect(draftAgentChoices([{ ...custom, name: 'My workspace agent' }], [catalog], [inspection]).map(choice => choice.hostId)).toEqual(['pi', 'pi'])
  })

  it('launches a selected legacy disabled custom profile preserving every launch setting', async () => {
    const { controller, transport } = setup()
    const previous = { ...config, enabled: false, args: ['--custom', 'with spaces'], env: { SECRET: 'preserve' } }
    transport.resolve('listAgentConfigs', [previous]).resolve('saveAgentConfig', { ...previous, enabled: true })
    controller.start(); await flush()
    expect(transport.callsFor('saveAgentConfig')[0].input).toEqual({ id: previous.id, name: previous.name, host_id: previous.host_id, protocol: previous.protocol, command: previous.command, args: previous.args, env: previous.env, enabled: true })
    expect(get(controller).phase).toBe('ready')
    await controller.close()
  })

  it('resolves a selected catalog profile by id with launch intent instead of a separate enable step', async () => {
    const { controller, transport } = setup()
    const linked = { ...config, enabled: false, catalog_id: 'pi' }
    transport.resolve('listAgentConfigs', [linked]).resolve('resolveCatalogAgent', { ...linked, enabled: true })
    controller.start(); await flush()
    expect(transport.callsFor('resolveCatalogAgent')[0].input).toEqual({ agent_id: 'pi', agent_config_id: 'config', enable: true })
    expect(transport.callsFor('saveAgentConfig')).toHaveLength(0)
    expect(get(controller).phase).toBe('ready')
    await controller.close()
  })

  it('offers an installed catalog entry but saves it only after selecting it', async () => {
    const { controller, transport, storage } = setup()
    storage.save('empty', { choice: '', cwd: '/repo', text: 'Task' })
    const draft = createDraftManagedSessionController(transport, 'empty', storage, vi.fn())
    transport.resolve('listAgentConfigs', []).resolve('listAvailableAgents', [catalog]).resolve('inspectAgentInstallation', inspection).resolve('resolveCatalogAgent', { ...config, catalog_id: 'pi' })
    draft.start(); await draft.refreshChoices(true); await flush()
    expect(get(draft).choices.map((choice) => choice.key)).toEqual(['catalog:pi'])
    expect(transport.callsFor('resolveCatalogAgent')).toHaveLength(0)
    draft.select('catalog:pi', '/repo'); await flush()
    expect(transport.callsFor('resolveCatalogAgent')[0].input).toEqual({ agent_id: 'pi', enable: true })
    expect(get(draft).choice).toBe('config:config')
    expect(get(draft).phase).toBe('ready')
    await draft.close()
    await controller.close()
  })

  it('offers missing agents without installing or starting them on selection', async () => {
    const { controller, transport, storage } = setup()
    storage.save('empty', { choice: '', cwd: '/repo', text: 'Keep my task' })
    const draft = createDraftManagedSessionController(transport, 'empty', storage, vi.fn())
    transport.resolve('listAgentConfigs', []).resolve('listAvailableAgents', [catalog])
      .resolve('inspectAgentInstallation', { ...inspection, source: 'missing', command: null })
    draft.start(); await draft.refreshChoices(true); await flush()
    draft.select('catalog:pi', '/repo'); await flush()
    expect(get(draft)).toMatchObject({ phase: 'idle', text: 'Keep my task', choice: 'catalog:pi' })
    expect(canPrepareAgentConnection(get(draft).choices[0])).toBe(true)
    expect(transport.callsFor('installAgent')).toHaveLength(0)
    expect(transport.callsFor('resolveCatalogAgent')).toHaveLength(0)
    expect(transport.callsFor('prepareManagedSession')).toHaveLength(0)
    await draft.close(); await controller.close()
  })

  it('prepares a bridge once on explicit action and continues the original draft', async () => {
    const { controller, transport, storage } = setup()
    storage.save('empty', { choice: '', cwd: '/repo', text: 'Keep my task' })
    const draft = createDraftManagedSessionController(transport, 'empty', storage, vi.fn())
    const installing = deferred<AgentInstallJob>()
    transport.resolve('listAgentConfigs', []).resolve('listAvailableAgents', [catalog])
      .resolve('inspectAgentInstallation', { ...inspection, source: 'missing', command: null })
      .handle('installAgent', () => installing.promise)
      .resolve('resolveCatalogAgent', { ...config, catalog_id: 'pi' })
    draft.start(); await draft.refreshChoices(true); await flush()
    draft.select('catalog:pi', '/repo'); await flush()
    const task = draft.prepareConnection()
    expect(draft.prepareConnection()).toBe(task)
    expect(get(draft).preparingConnection).toBe(true)
    draft.edit('Still editing during setup')
    transport.resolve('inspectAgentInstallation', inspection)
    installing.resolve({ id: 'install', agent_id: 'pi', phase: 'complete', messages: [], result: null, cancel_requested: false })
    await task; await flush()
    expect(transport.callsFor('installAgent')).toHaveLength(1)
    expect(transport.callsFor('sendManagedPrompt')).toHaveLength(0)
    expect(get(draft)).toMatchObject({ phase: 'ready', preparingConnection: false, text: 'Still editing during setup', cwd: '/repo', choice: 'config:config' })
    await draft.close(); await controller.close()
  })

  it('keeps native installation and missing runtime as guidance instead of a managed install', () => {
    const missing = { ...inspection, source: 'missing' as const, command: null }
    const native = draftAgentChoices([], [{ ...catalog, connection_kind: 'native' }], [missing])[0]
    expect(agentNeedsPreparation(native)).toBe(true)
    expect(canPrepareAgentConnection(native)).toBe(false)
    const runtimeMissing = draftAgentChoices([], [catalog], [{ ...missing, checks: [{ id: 'node', status: 'fail', message: 'Node.js unavailable' }] }])[0]
    expect(canPrepareAgentConnection(runtimeMissing)).toBe(false)
  })

  it('does not start connection installation when npm is unavailable but reported as a runtime warning', async () => {
    const { controller, transport, storage } = setup()
    storage.save('empty', { choice: '', cwd: '/repo', text: 'Keep my task' })
    const draft = createDraftManagedSessionController(transport, 'empty', storage, vi.fn())
    transport.resolve('listAgentConfigs', []).resolve('listAvailableAgents', [catalog])
      .resolve('inspectAgentInstallation', { ...inspection, source: 'missing', command: null,
        checks: [{ id: 'node', status: 'pass', message: 'Node.js available' }, { id: 'npm', status: 'warn', message: 'npm is unavailable; existing agents can still run' }] })
    draft.start(); await draft.refreshChoices(true); await flush()
    draft.select('catalog:pi', '/repo'); await flush()
    expect(agentNeedsPreparation(get(draft).choices[0])).toBe(true)
    expect(canPrepareAgentConnection(get(draft).choices[0])).toBe(false)
    await draft.prepareConnection()
    expect(transport.callsFor('installAgent')).toHaveLength(0)
    expect(transport.callsFor('prepareManagedSession')).toHaveLength(0)
    expect(get(draft)).toMatchObject({ phase: 'idle', preparingConnection: false, text: 'Keep my task' })
    await draft.close(); await controller.close()
  })

  it('offers preparation for a missing packaged Pi dependency but leaves external dependencies to the agent guide', () => {
    const requiredPi = { command: 'pi', required: true, package: '@earendil-works/pi-coding-agent', pinned_version: '0.83.0', instructions: 'Install Pi' }
    const missingPi: AgentInspection = { ...inspection,
      dependencies: [{ command: 'pi', required: true, path: null, version: null }],
      checks: [{ id: 'node', status: 'pass', message: '' }, { id: 'npm', status: 'pass', message: '' }, { id: 'dependency_pi', status: 'fail', message: 'Pi not found' }] }
    const choice = draftAgentChoices([], [{ ...catalog, dependencies: [requiredPi] }], [missingPi])[0]
    expect(agentNeedsPreparation(choice)).toBe(true)
    expect(canPrepareAgentConnection(choice)).toBe(true)
    const external = draftAgentChoices([], [{ ...catalog, dependencies: [{ ...requiredPi, package: null, pinned_version: null }] }], [missingPi])[0]
    expect(agentNeedsPreparation(external)).toBe(true)
    expect(canPrepareAgentConnection(external)).toBe(false)
    expect(canPrepareAgentConnection({ ...choice, entry: { ...choice.entry!, verification: { status: 'unsupported', versions: [], note: '' } } })).toBe(false)
    expect(canPrepareAgentConnection({ ...choice, config })).toBe(false)
  })

  it('keeps the selected agent when another page materializes its launch configuration', async () => {
    const { controller, transport, storage } = setup()
    storage.save('empty', { choice: '', cwd: '', text: 'Keep my task' })
    const draft = createDraftManagedSessionController(transport, 'empty', storage, vi.fn())
    transport.resolve('listAgentConfigs', []).resolve('listAvailableAgents', [catalog]).resolve('inspectAgentInstallation', inspection)
    draft.start(); await draft.refreshChoices(true); await flush()
    draft.select('catalog:pi', '')
    transport.resolve('listAgentConfigs', [{ ...config, catalog_id: 'pi' }])
    await draft.refreshChoices()
    expect(get(draft)).toMatchObject({ choice: 'config:config', text: 'Keep my task', cwd: '' })
    expect(transport.callsFor('prepareManagedSession')).toHaveLength(0)
    await draft.close(); await controller.close()
  })

  it('does not start an agent after preparation completes for a closed draft', async () => {
    const { controller, transport, storage } = setup()
    storage.save('empty', { choice: '', cwd: '/repo', text: 'Keep my task' })
    const draft = createDraftManagedSessionController(transport, 'empty', storage, vi.fn())
    const installing = deferred<AgentInstallJob>()
    transport.resolve('listAgentConfigs', []).resolve('listAvailableAgents', [catalog])
      .resolve('inspectAgentInstallation', { ...inspection, source: 'missing', command: null })
      .handle('installAgent', () => installing.promise)
    draft.start(); await draft.refreshChoices(true); await flush()
    draft.select('catalog:pi', '/repo'); await flush()
    const task = draft.prepareConnection()
    await draft.close()
    installing.resolve({ id: 'install', agent_id: 'pi', phase: 'complete', messages: [], result: null, cancel_requested: false })
    await task
    expect(transport.callsFor('prepareManagedSession')).toHaveLength(0)
    expect(storage.load('empty').text).toBe('Keep my task')
    await controller.close()
  })

  it('preserves every saved profile while grouping additional launch profiles as advanced', () => {
    const linked = { ...config, catalog_id: 'pi' }
    const extra = { ...linked, id: 'other', name: 'Other account', env: { SECRET: 'retained' } }
    const choices = draftAgentChoices([linked, extra], [catalog], [inspection])
    expect(choices.map(choice => choice.key)).toEqual(['config:config', 'config:other'])
    expect(choices[0].advanced).toBe(false)
    expect(choices[1]).toMatchObject({ advanced: true, config: { env: { SECRET: 'retained' } } })
  })
})

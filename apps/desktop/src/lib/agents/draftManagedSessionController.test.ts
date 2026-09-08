import { get } from 'svelte/store'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import type { AgentCatalogEntry, AgentConfig, AgentInspection, AgentInstallJob, ManagedSessionSnapshot } from '$lib/generated/feedback'
import { agentNeedsPreparation, canPrepareAgentConnection, createDraftManagedSessionController, draftAgentChoices } from './draftManagedSessionController'
import { createManagedSessionDraftStorage } from './managedSessionDrafts'
import { readAgentDetectionCache, rememberAgentInspection } from './agentDetectionCache'
import { configureClientDiagnostics, type ClientDiagnosticEvent } from '$lib/diagnostics/clientDiagnostics'

let stopDiagnostics: (() => void) | undefined
function captureDiagnostics() {
  const events: ClientDiagnosticEvent[] = []
  stopDiagnostics = configureClientDiagnostics(event => { events.push(event) })
  return events
}
afterEach(() => { stopDiagnostics?.(); stopDiagnostics = undefined })

const config: AgentConfig = { id: 'config', name: 'Pi', host_id: 'pi', protocol: 'acp', enabled: true, command: 'pi-acp', args: [], env: {}, created_at: '', updated_at: '' }
const catalog: AgentCatalogEntry = { id: 'pi', name: 'Pi', host_id: 'pi', description: '', connection_kind: 'bridge', distribution: { kind: 'npm', package: 'pi-acp', command: 'pi-acp', pinned_version: '1.0.0', node_required: '22.0.0' }, args: [], dependencies: [], verification: { status: 'unverified', versions: [], note: '' } }
const inspection: AgentInspection = { agent_id: 'pi', command: 'pi-acp', args: [], source: 'system', version: '1.0.0', checks: [], dependencies: [] }
function snapshot(id: string, lifecycle: 'prepared' | 'active' = 'prepared', connection: 'connected' | 'failed' = 'connected'): ManagedSessionSnapshot {
  return { session: { session_id: id, host_id: 'pi', host_session_id: id, title: 'Task', lifecycle,
    management: { kind: 'managed', protocol: 'acp', agent_config_id: 'config', cwd: '/repo', remote_session_id: `remote-${id}` }, created_at: '', updated_at: '' },
    runtime: { configuration: { options: [] }, connection, activity: 'idle', instance_id: 'runtime', config_updated_at: null,
      capabilities: { prompt: { image: false, audio: false, embedded_context: true, resource_links: true }, load_session: false, resume_session: false, http_mcp: false }, last_error: connection === 'failed' ? 'Connection failed' : null },
    activities: [], interactions: [], deliveries: [], deleting: false, recovery: null }
}
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>((done) => { resolve = done }); return { promise, resolve } }
async function flush() { for (let i = 0; i < 35; i++) await Promise.resolve() }
function setup() {
  const data = new Map<string, string>()
  const storage = createManagedSessionDraftStorage({ getItem: (key) => data.get(key) ?? null, setItem: (key, value) => { data.set(key, value) } })
  storage.save('draft', { choice: 'config:config', cwd: '/repo', text: 'Draft task' })
  const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
    .resolve('listAgentConfigs', [config]).resolve('listAvailableAgents', []).resolve('discardPreparedSession', undefined)
    .resolve('prepareManagedSession', snapshot('one')).resolve('getManagedSession', snapshot('one'))
  const promoted = vi.fn()
  const controller = createDraftManagedSessionController(transport, 'draft', storage, promoted)
  return { transport, controller, promoted, storage, data }
}

describe('managed draft lifecycle', () => {
  it('does not treat an empty prepared snapshot as rejection after a lost first-message response', async () => {
    const { controller, transport, promoted } = setup()
    controller.start(); await flush()
    transport.reject('sendManagedPrompt', new Error('Response lost'))
    await expect(controller.send('Draft task')).rejects.toThrow('Response lost')
    expect(get(controller).awaitingAcknowledgement).toBe(true)
    await controller.retry()
    expect(get(controller).awaitingAcknowledgement).toBe(true)
    expect(transport.callsFor('startManagedSession')).toHaveLength(0)
    await expect(controller.close()).rejects.toThrow('Could not confirm')
    expect(transport.callsFor('discardPreparedSession')).toHaveLength(0)
    transport.resolve('getManagedSession', snapshot('one', 'active'))
    await controller.retry()
    expect(promoted).toHaveBeenCalledTimes(1)
    expect(transport.callsFor('sendManagedPrompt')).toHaveLength(1)
    await controller.close()
  })

  it('redacts failure evidence when it enters draft state, before environment settings can change', async () => {
    const { controller, transport } = setup()
    const failed = snapshot('one', 'prepared', 'failed')
    failed.runtime.last_error = 'Rejected old-secret-value'
    failed.runtime.failure = { stage: 'session', reason: 'authentication', message: 'Rejected old-secret-value' }
    transport.resolve('listAgentConfigs', [{ ...config, env: { API_KEY: 'old-secret-value' } }]).resolve('prepareManagedSession', failed)
    controller.start(); await flush()
    expect(get(controller).error).not.toContain('old-secret-value')
    expect(get(controller).failure?.message).not.toContain('old-secret-value')
    expect(get(controller).snapshot?.runtime.last_error).not.toContain('old-secret-value')
    expect(get(controller).snapshot?.runtime.failure?.message).not.toContain('old-secret-value')
    await controller.close()
  })
  it('retains the chosen project and message after authenticated session preparation fails, then retries only that session', async () => {
    const { controller, transport, storage } = setup()
    const failed = snapshot('one', 'prepared', 'failed')
    failed.runtime.failure = { stage: 'session', reason: 'authentication', message: 'Sign-in required by the connected agent' }
    transport.resolve('prepareManagedSession', failed).resolve('getManagedSession', failed).resolve('startManagedSession', snapshot('one'))
    controller.start(); await flush()
    expect(get(controller)).toMatchObject({ phase: 'failed', cwd: '/repo', text: 'Draft task', failure: { stage: 'session', reason: 'authentication' } })
    expect(storage.load('draft')).toMatchObject({ cwd: '/repo', text: 'Draft task', choice: 'config:config' })
    await controller.retry()
    expect(transport.callsFor('prepareManagedSession')).toHaveLength(1)
    expect(transport.callsFor('startManagedSession').map(call => call.input)).toEqual([{ session_id: 'one' }])
    expect(transport.callsFor('sendManagedPrompt')).toHaveLength(0)
    expect(get(controller)).toMatchObject({ phase: 'ready', text: 'Draft task', failure: null })
    await controller.close()
  })

  it('keeps confirmed model selection and reports a rejected configuration beside the same draft', async () => {
    const { controller, transport } = setup()
    const ready = snapshot('one')
    ready.runtime.configuration = { options: [{ id: 'model', name: 'Model', description: null, category: 'model',
      kind: { type: 'select', current_value: 'current-model', options: [{ value: 'current-model', name: 'Current model', description: null, group: null }] } }] }
    transport.resolve('prepareManagedSession', ready)
    controller.start(); await flush()
    const failed = structuredClone(ready)
    failed.runtime.failure = { stage: 'configuration', reason: 'model', message: 'Requested model is unavailable' }
    failed.runtime.last_error = failed.runtime.failure.message
    transport.resolve('getManagedSession', failed).reject('setManagedSessionConfig', new Error('Requested model is unavailable'))
    await expect(controller.configure({ config_id: 'model', value: { type: 'select', value: 'missing-model' } })).rejects.toThrow('unavailable')
    expect(get(controller)).toMatchObject({ phase: 'ready', cwd: '/repo', text: 'Draft task', failure: { stage: 'configuration', reason: 'model' } })
    expect(get(controller).snapshot?.runtime.configuration).toEqual(ready.runtime.configuration)
    expect(transport.callsFor('sendManagedPrompt')).toHaveLength(0)
    await controller.close()
  })
  it.each(['', '  ', 'relative/project'])('requires an explicit absolute project directory before preparation or submission: %j', async cwd => {
    const { controller, transport, storage, promoted } = setup()
    controller.select('config:config', cwd)
    controller.start(); await flush()
    expect(get(controller)).toMatchObject({ phase: 'idle', snapshot: null, cwd, text: 'Draft task' })
    expect(transport.callsFor('prepareManagedSession')).toHaveLength(0)
    await controller.retry()
    expect(get(controller).error).toBe(cwd.trim() ? 'Enter an absolute project directory.' : 'Choose a project directory before connecting.')
    await controller.send('Draft task')
    expect(transport.callsFor('prepareManagedSession')).toHaveLength(0)
    expect(transport.callsFor('sendManagedPrompt')).toHaveLength(0)
    expect(transport.callsFor('createManagedSession')).toHaveLength(0)
    expect(promoted).not.toHaveBeenCalled()
    expect(storage.load('draft')).toMatchObject({ cwd, text: 'Draft task' })
    controller.select('config:config', 'D:\\projects\\chosen'); await flush()
    expect(transport.callsFor('prepareManagedSession').map(call => call.input)).toEqual([{ agent_config_id: 'config', cwd: 'D:\\projects\\chosen' }])
    expect(get(controller)).toMatchObject({ phase: 'ready', error: '', text: 'Draft task' })
    await controller.close()
  })

  it('discards an in-flight connection when changing agent and clearing the directory, preserving the typed draft', async () => {
    const { controller, transport, storage } = setup()
    const pending = deferred<ManagedSessionSnapshot>()
    transport.resolve('listAgentConfigs', [config, { ...config, id: 'another', name: 'Another agent' }])
      .handle('prepareManagedSession', () => pending.promise)
    controller.start(); await flush()
    controller.edit('Keep this task while switching')
    controller.select('config:another', '')
    pending.resolve(snapshot('one')); await flush()
    expect(transport.callsFor('discardPreparedSession').map(call => call.input)).toEqual([{ session_id: 'one' }])
    expect(transport.callsFor('prepareManagedSession')).toHaveLength(1)
    expect(get(controller)).toMatchObject({ phase: 'idle', choice: 'config:another', cwd: '', snapshot: null, text: 'Keep this task while switching' })
    expect(storage.load('draft')).toMatchObject({ choice: 'config:another', cwd: '', text: 'Keep this task while switching' })
    await controller.send('Keep this task while switching')
    expect(transport.callsFor('sendManagedPrompt')).toHaveLength(0)
    transport.resolve('prepareManagedSession', snapshot('two'))
    controller.select('config:another', '/another-project'); await flush()
    expect(transport.callsFor('prepareManagedSession')[1].input).toEqual({ agent_config_id: 'another', cwd: '/another-project' })
    expect(get(controller).text).toBe('Keep this task while switching')
    await controller.close()
  })

  it.each(['pi-acp', 'private-draft-catalog-canary'])('records safe selected agent identity for %s', async catalogId => {
    const events = captureDiagnostics()
    const { controller, transport } = setup()
    const selected = { ...config, catalog_id: catalogId }
    transport.resolve('listAgentConfigs', [selected]).resolve('resolveCatalogAgent', selected).resolve('sendManagedPrompt', snapshot('one', 'active'))
    controller.start(); await flush()
    await controller.send('private-prompt-canary')
    const agent = catalogId === 'pi-acp' ? 'pi-acp' : 'custom'
    for (const action of ['prepare', 'send', 'promote']) {
      expect(events.find(event => event.details?.action === action && event.outcome === 'ok')).toMatchObject({ details: { agent } })
    }
    expect(JSON.stringify(events)).not.toMatch(/private-draft-catalog-canary|private-prompt-canary/u)
    await controller.close()
  })

  it('records preparation, failed first send and successful promotion without recording task or error contents', async () => {
    const events = captureDiagnostics()
    const { controller, transport } = setup()
    controller.start(); await flush()
    expect(events.find(event => event.activity === 'session_draft' && event.details?.action === 'prepare' && event.outcome === 'ok')).toMatchObject({ details: { status: 'connected' }, durationMs: expect.any(Number) })
    transport.reject('sendManagedPrompt', Object.assign(new Error('secret-error-canary C:/private/repo'), { code: 'MANAGED_SESSION_BUSY', retryable: false }))
    await expect(controller.send('secret-prompt-canary')).rejects.toThrow('secret-error-canary')
    expect(events.find(event => event.details?.action === 'send' && event.outcome === 'failed')).toMatchObject({ details: { promoted: false, reason: 'acknowledgement_lost' } })
    transport.resolve('startManagedSession', snapshot('one')).resolve('sendManagedPrompt', snapshot('one', 'active'))
    await controller.retry()
    await controller.send('secret-prompt-canary')
    expect(events.find(event => event.details?.action === 'promote' && event.outcome === 'ok')).toBeDefined()
    expect(events.find(event => event.details?.action === 'send' && event.outcome === 'ok')).toMatchObject({ details: { promoted: true } })
    expect(JSON.stringify(events)).not.toMatch(/secret-prompt-canary|secret-error-canary|Draft task|\/repo/u)
    await controller.close()
  })

  it('records obsolete preparation as cancelled and keeps diagnostics sinks outside the preparation await chain', async () => {
    const events = captureDiagnostics()
    const { controller, transport } = setup()
    const pending = deferred<ManagedSessionSnapshot>()
    transport.handle('prepareManagedSession', () => pending.promise)
    controller.start(); await flush()
    const close = controller.close()
    pending.resolve(snapshot('one'))
    await close
    expect(events.find(event => event.details?.action === 'prepare' && event.outcome === 'cancelled')).toMatchObject({ details: { reason: 'stale' } })
    stopDiagnostics?.()
    stopDiagnostics = configureClientDiagnostics(() => new Promise<void>(() => {}))
    const fresh = setup()
    fresh.controller.start(); await flush()
    expect(get(fresh.controller).phase).toBe('ready')
    await fresh.controller.close()
  })
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

  it.each(['complete', 'failed', 'cancelled'] as const)('polls connection preparation to %s without losing the task', async (phase) => {
    vi.useFakeTimers()
    const { controller, transport, storage } = setup()
    storage.save('empty', { choice: '', cwd: '/repo', text: 'Keep my task' })
    const draft = createDraftManagedSessionController(transport, 'empty', storage, vi.fn())
    const job: AgentInstallJob = { id: 'install', agent_id: 'pi', phase: 'installing', messages: [], result: null, cancel_requested: false }
    transport.resolve('listAgentConfigs', []).resolve('listAvailableAgents', [catalog])
      .resolve('inspectAgentInstallation', { ...inspection, source: 'missing', command: null })
      .resolve('installAgent', job).resolve('listAgentInstallJobs', [{ ...job, phase }])
      .resolve('cancelAgentInstall', undefined).resolve('resolveCatalogAgent', { ...config, catalog_id: 'pi' })
    try {
      draft.start(); await draft.refreshChoices(true); await flush()
      draft.select('catalog:pi', '/repo'); await flush()
      const task = draft.prepareConnection(); await flush()
      if (phase === 'cancelled') {
        await draft.cancelPreparation()
        expect(get(draft).installationJob?.cancel_requested).toBe(true)
        expect(transport.callsFor('cancelAgentInstall')[0].input).toEqual({ job_id: 'install' })
      }
      if (phase === 'complete') transport.resolve('inspectAgentInstallation', inspection)
      await vi.advanceTimersByTimeAsync(500); await task
      expect(get(draft)).toMatchObject({ preparingConnection: false, text: 'Keep my task' })
      expect(get(draft).installationJob?.phase).toBe(phase)
      expect(transport.callsFor('listAgentInstallJobs')).toHaveLength(1)
      expect(transport.callsFor('sendManagedPrompt')).toHaveLength(0)
      if (phase === 'complete') expect(get(draft).phase).toBe('ready')
      else {
        expect(get(draft).error).not.toBe('')
        expect(transport.callsFor('prepareManagedSession')).toHaveLength(0)
      }
    } finally { await draft.close(); await controller.close(); vi.useRealTimers() }
  })

  it('releases the preparation lock when closing fails during installation polling', async () => {
    vi.useFakeTimers()
    const { controller, transport } = setup()
    const job: AgentInstallJob = { id: 'install', agent_id: 'pi', phase: 'installing', messages: [], result: null, cancel_requested: false }
    transport.resolve('listAvailableAgents', [catalog]).resolve('inspectAgentInstallation', { ...inspection, source: 'missing', command: null })
      .resolve('installAgent', job)
    try {
      controller.start(); await flush()
      await controller.refreshChoices(true)
      transport.handle('discardPreparedSession', () => { throw new Error('Cleanup failed') })
      controller.select('catalog:pi', '/repo'); await flush()
      expect(get(controller).phase).toBe('failed')
      const task = controller.prepareConnection(); await flush()
      let rejectCleanup!: (cause: Error) => void
      transport.handle('discardPreparedSession', () => new Promise<void>((_, reject) => { rejectCleanup = reject }))
      const closing = controller.close()
      const rejected = expect(closing).rejects.toThrow('Cleanup failed again')
      await flush(); await vi.advanceTimersByTimeAsync(500); await task
      rejectCleanup(new Error('Cleanup failed again')); await rejected
      expect(get(controller)).toMatchObject({ phase: 'failed', preparingConnection: false, text: 'Draft task' })
    } finally { transport.resolve('discardPreparedSession', undefined); await controller.close(); vi.useRealTimers() }
  })

  it('preserves every saved profile while grouping additional launch profiles as advanced', () => {
    const linked = { ...config, catalog_id: 'pi' }
    const extra = { ...linked, id: 'other', name: 'Other account', env: { SECRET: 'retained' } }
    const choices = draftAgentChoices([linked, extra], [catalog], [inspection])
    expect(choices.map(choice => choice.key)).toEqual(['config:config', 'config:other'])
    expect(choices[0].advanced).toBe(false)
    expect(choices[1]).toMatchObject({ advanced: true, config: { env: { SECRET: 'retained' } } })
  })
  it('prepares automatically, retains editable input, and never creates an active session', async () => {
    const { controller, transport } = setup()
    controller.start(); await flush()
    expect(get(controller).phase).toBe('ready')
    expect(get(controller).text).toBe('Draft task')
    expect(transport.callsFor('prepareManagedSession')[0].input).toEqual({ agent_config_id: 'config', cwd: '/repo' })
    expect(transport.callsFor('createManagedSession')).toHaveLength(0)
    await controller.close()
  })

  it('cleans stale prepare before preparing the latest directory and retains text', async () => {
    const { controller, transport } = setup()
    const first = deferred<ManagedSessionSnapshot>()
    transport.handle('prepareManagedSession', ({ cwd }) => cwd === '/repo' ? first.promise : snapshot('two'))
    controller.start(); await flush()
    controller.select('config:config', '/new')
    controller.edit('Still editing')
    first.resolve(snapshot('one')); await flush()
    const calls = transport.calls.filter((call) => ['prepareManagedSession', 'discardPreparedSession'].includes(call.name))
    expect(calls.map((call) => call.name)).toEqual(['prepareManagedSession', 'discardPreparedSession', 'prepareManagedSession'])
    expect(get(controller).snapshot?.session.session_id).toBe('two')
    expect(get(controller).text).toBe('Still editing')
    await controller.close()
  })

  it('discards a late preparation when the tab closes before receiving its identity', async () => {
    const { controller, transport } = setup()
    const first = deferred<ManagedSessionSnapshot>()
    transport.handle('prepareManagedSession', () => first.promise)
    controller.start(); await flush()
    const closed = controller.close()
    first.resolve(snapshot('late')); await closed
    expect(transport.callsFor('discardPreparedSession').map((call) => call.input)).toEqual([{ session_id: 'late' }])
    expect(get(controller).snapshot).toBeNull()
  })

  it('keeps ownership after discard failure and retries cleanup before any replacement', async () => {
    const { controller, transport } = setup()
    controller.start(); await flush()
    transport.handle('discardPreparedSession', () => { throw new Error('Cleanup failed') })
    controller.select('config:config', '/new'); await flush()
    expect(get(controller).error).toBe('Cleanup failed')
    expect(transport.callsFor('prepareManagedSession')).toHaveLength(1)
    transport.resolve('discardPreparedSession', undefined).resolve('prepareManagedSession', snapshot('two'))
    await controller.retry()
    expect(get(controller).snapshot?.session.session_id).toBe('two')
    await controller.close()
  })

  it('retries a failed preparation with the same remote session identity', async () => {
    const { controller, transport } = setup()
    transport.resolve('prepareManagedSession', snapshot('one', 'prepared', 'failed')).resolve('startManagedSession', snapshot('one'))
    controller.start(); await flush()
    expect(get(controller).phase).toBe('failed')
    await controller.retry()
    expect(get(controller).phase).toBe('ready')
    expect(transport.callsFor('prepareManagedSession')).toHaveLength(1)
    expect(transport.callsFor('startManagedSession')[0].input).toEqual({ session_id: 'one' })
    await controller.close()
  })

  it('promotes once from the accepted first prompt and closing then only closes the view', async () => {
    const { controller, transport, promoted, storage } = setup()
    controller.start(); await flush()
    transport.resolve('sendManagedPrompt', snapshot('one', 'active'))
    await controller.send('Draft task')
    expect(transport.callsFor('sendManagedPrompt')[0].input).toEqual({ session_id: 'one', text: 'Draft task' })
    expect(promoted).toHaveBeenCalledTimes(1)
    expect(get(controller).text).toBe('')
    expect(storage.load('draft').text).toBe('')
    expect(await controller.close()).toBe('one')
    expect(transport.callsFor('discardPreparedSession')).toHaveLength(0)
  })

  it('promotes from an invalidation before a delayed acknowledgement, preserving subsequent text', async () => {
    const { controller, transport, promoted } = setup()
    controller.start(); await flush()
    const sending = deferred<ManagedSessionSnapshot>()
    transport.handle('sendManagedPrompt', () => sending.promise)
    const sent = controller.send('Draft task'); await flush()
    controller.edit('Next task')
    transport.resolve('getManagedSession', snapshot('one', 'active'))
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'invalidate', runtime_generation: 'runtime', revision: '1', resources: [{ kind: 'managed_session', session_id: 'one' }] })
    await flush()
    expect(promoted).toHaveBeenCalledTimes(1)
    expect(get(controller).text).toBe('Next task')
    sending.resolve(snapshot('one', 'active')); await sent
    expect(promoted).toHaveBeenCalledTimes(1)
    await controller.close()
  })

  it('restores input using a fresh preparation, with no persisted runtime identity', async () => {
    const { controller, transport, storage, data } = setup()
    controller.start(); await flush()
    controller.edit('Saved work')
    const raw = [...data.values()].join('')
    expect(raw).not.toContain('remote-one')
    expect(raw).not.toContain('session_id')
    await controller.close()
    transport.resolve('prepareManagedSession', snapshot('two'))
    const restored = createDraftManagedSessionController(transport, 'draft', storage, vi.fn())
    restored.start(); await flush()
    expect(get(restored).text).toBe('Saved work')
    expect(get(restored).snapshot?.session.session_id).toBe('two')
    await restored.close()
  })

  it('checks uncertain acceptance before retry and promotes without sending the message again', async () => {
    const { controller, transport, promoted } = setup()
    controller.start(); await flush()
    transport.handle('sendManagedPrompt', () => { throw new Error('Response lost') })
    transport.handle('getManagedSession', () => { throw new Error('Offline') })
    await expect(controller.send('Draft task')).rejects.toThrow('Response lost')
    expect(get(controller).awaitingAcknowledgement).toBe(true)
    controller.select('config:config', '/another')
    expect(get(controller).cwd).toBe('/repo')
    transport.resolve('getManagedSession', snapshot('one', 'active'))
    await controller.retry()
    expect(promoted).toHaveBeenCalledTimes(1)
    expect(transport.callsFor('sendManagedPrompt')).toHaveLength(1)
    expect(transport.callsFor('startManagedSession')).toHaveLength(0)
    await controller.close()
    expect(transport.callsFor('discardPreparedSession')).toHaveLength(0)
  })

  it('restores a rejected first task and does not overwrite text typed during a later attempt', async () => {
    const { controller, transport } = setup()
    controller.start(); await flush()
    transport.handle('sendManagedPrompt', () => { throw Object.assign(new Error('Not accepted'), { code: 'MANAGED_SESSION_NOT_CONNECTED', retryable: false }) })
    await expect(controller.send('Draft task')).rejects.toThrow('Not accepted')
    expect(get(controller).text).toBe('Draft task')
    expect(get(controller).awaitingAcknowledgement).toBe(false)
    transport.resolve('startManagedSession', snapshot('one'))
    await controller.retry()
    const response = deferred<ManagedSessionSnapshot>()
    transport.handle('sendManagedPrompt', () => response.promise)
    const sending = controller.send('Draft task'); await flush()
    expect(get(controller).text).toBe('')
    controller.edit('Draft task')
    response.resolve(snapshot('one', 'active'))
    await sending
    expect(get(controller).text).toBe('Draft task')
    await controller.close()
  })

  it('rebuilds an unsent connection after its selected profile changes in settings', async () => {
    const { controller, transport } = setup()
    controller.start(); await flush()
    transport.resolve('listAgentConfigs', [{ ...config, updated_at: 'later', env: { MODEL: 'new' } }]).resolve('prepareManagedSession', snapshot('two'))
    await controller.refreshChoices(); await flush()
    expect(transport.callsFor('discardPreparedSession').map((call) => call.input)).toEqual([{ session_id: 'one' }])
    expect(get(controller).snapshot?.session.session_id).toBe('two')
    expect(get(controller).text).toBe('Draft task')
    await controller.close()
  })

  it('does not discard a session promoted while close waits for the first send', async () => {
    const { controller, transport, promoted } = setup()
    controller.start(); await flush()
    const response = deferred<ManagedSessionSnapshot>()
    transport.handle('sendManagedPrompt', () => response.promise)
    const send = controller.send('Draft task'); await flush()
    const close = controller.close()
    response.resolve(snapshot('one', 'active'))
    await send
    expect(await close).toBe('one')
    expect(promoted).toHaveBeenCalledTimes(1)
    expect(transport.callsFor('discardPreparedSession')).toHaveLength(0)
  })

  it('never restores the old configuration from an event while a directory change is pending', async () => {
    const { controller, transport } = setup()
    controller.start(); await flush()
    controller.select('config:config', '/changed', 500)
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'invalidate', runtime_generation: 'runtime', revision: '1', resources: [{ kind: 'managed_session', session_id: 'one' }] })
    await flush()
    expect(get(controller).snapshot).toBeNull()
    expect(transport.callsFor('getManagedSession')).toHaveLength(0)
    await controller.close()
  })
})

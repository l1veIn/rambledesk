import { get } from 'svelte/store'
import { describe, expect, it, vi } from 'vitest'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import type { AgentCatalogEntry, AgentConfig } from '$lib/generated/feedback'
import { createDraftManagedSessionController, draftAgentChoices } from './draftManagedSessionController'
import { agentListItems } from './agentCatalogController'
import { agentLaunchSignature, forgetAgentConnection, readAgentDetectionCache, rememberAgentConnection, rememberAgentInspection } from './agentDetectionCache'
import { catalog, config, deferred, flush, inspection, setup, snapshot } from './draftManagedSessionControllerTestHarness'

const connected = { ok: true, message: 'ACP connected', details: [] }
function checks(...configs: AgentConfig[]) {
  return Object.fromEntries(configs.map(config => [config.id, { signature: agentLaunchSignature(config), result: connected }]))
}
describe('new session checked agent choices', () => {
  it('reuses completed checks on mount, refresh, remount and runtime restart without probing', async () => {
    const { controller, transport, storage } = setup()
    transport.resolve('listAvailableAgents', [catalog])
    rememberAgentInspection(transport, inspection)
    controller.start(); await flush()
    expect(get(controller).choices.map(choice => choice.key)).toEqual(['config:config'])
    expect(get(controller).phase).toBe('ready')
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'ready', runtime_generation: 'first', revision: '1' })
    await controller.refreshChoices(false)
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'ready', runtime_generation: 'restarted', revision: '1' })
    await flush(); await controller.close()
    const reopened = createDraftManagedSessionController(transport, 'draft', storage, vi.fn())
    reopened.start(); await flush()
    expect(get(reopened).choices.map(choice => choice.key)).toEqual(['config:config'])
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(0)
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(0)
    await reopened.close()
  })
  it('makes checked profiles usable while the catalog is still loading', async () => {
    const { controller, transport } = setup()
    const listing = deferred<AgentCatalogEntry[]>()
    transport.handle('listAvailableAgents', () => listing.promise)
    controller.start(); await flush()
    expect(get(controller)).toMatchObject({ phase: 'ready', choice: 'config:config', loadingChoices: false })
    await controller.close(); listing.resolve([catalog]); await flush()
  })
  it('hides unchecked, failed, disabled and edited profiles and unchecked catalog entries', () => {
    const failed = { ...config, id: 'failed' }, unchecked = { ...config, id: 'unchecked' }
    const disabled = { ...config, id: 'disabled', enabled: false }, edited = { ...config, id: 'edited' }
    const connections = checks(config, failed, disabled, edited)
    connections.failed.result = { ...connected, ok: false }
    const choices = draftAgentChoices([config, failed, unchecked, disabled, { ...edited, env: { TOKEN: 'changed' } }], [catalog], [inspection], connections)
    expect(choices.map(choice => choice.key)).toEqual(['config:config'])
    expect(draftAgentChoices([], [catalog], [inspection])).toEqual([])
  })
  it('does not revive or launch a selected unchecked legacy configuration', async () => {
    const { controller, transport } = setup()
    forgetAgentConnection(transport, config.id)
    transport.resolve('listAgentConfigs', [{ ...config, enabled: false, command: 'old-cli' }])
    controller.start(); await flush()
    expect(get(controller)).toMatchObject({ choices: [], choice: '', phase: 'idle', text: 'Draft task' })
    for (const command of ['saveAgentConfig', 'resolveCatalogAgent', 'prepareManagedSession'] as const) expect(transport.callsFor(command)).toHaveLength(0)
    await controller.close()
  })
  it('updates an open draft when settings check a previously hidden profile, retaining all saved profiles', async () => {
    const { controller, transport } = setup()
    const alternate = { ...config, id: 'alternate' }
    transport.resolve('listAgentConfigs', [config, alternate])
    controller.start(); await flush()
    expect(get(controller).choices.map(choice => choice.key)).toEqual(['config:config'])
    rememberAgentConnection(transport, alternate, connected); await flush()
    expect(get(controller).choices.map(choice => choice.key)).toEqual(['config:config', 'config:alternate'])
    expect(get(controller).choice).toBe('config:config')
    expect(transport.callsFor('prepareManagedSession')).toHaveLength(1)
    forgetAgentConnection(transport, alternate.id)
    expect(get(controller).choices.map(choice => choice.key)).toEqual(['config:config'])
    await controller.close(); rememberAgentConnection(transport, alternate, connected)
    expect(get(controller).choices.map(choice => choice.key)).toEqual(['config:config'])
  })
  it('detects and checks explicitly, offering only successes without selecting or installing an agent', async () => {
    const { controller, transport, storage } = setup()
    const passed = { ...config, catalog_id: 'pi', id: 'passed' }, failed = { ...config, id: 'failed' }
    let configs = [failed]
    transport.handle('listAgentConfigs', () => configs).resolve('listAvailableAgents', [catalog])
      .resolve('listAgentInstallJobs', []).resolve('inspectAgentInstallation', inspection)
      .handle('resolveCatalogAgent', () => { configs = [...configs, passed]; return passed })
      .handle('checkAgentConfig', ({ agent_config_id }) => ({ ...connected, ok: agent_config_id === passed.id }))
    storage.save('empty', { choice: '', cwd: '/repo', text: 'Keep my task' })
    const draft = createDraftManagedSessionController(transport, 'empty', storage, vi.fn())
    draft.start(); await flush()
    expect(get(draft).choices).toEqual([])
    const scan = draft.refreshChoices(true)
    expect(draft.refreshChoices(true)).toBe(scan)
    await scan; await flush()
    expect(get(draft).choices.map(choice => choice.key)).toEqual(['config:passed'])
    expect(get(draft)).toMatchObject({ choice: '', text: 'Keep my task', phase: 'idle' })
    expect(transport.callsFor('inspectAgentInstallation')).toHaveLength(1)
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(2)
    expect(transport.callsFor('installAgent')).toHaveLength(0)
    transport.resolve('prepareManagedSession', snapshot('selected'))
    draft.select('config:passed', '/repo'); await flush()
    expect(transport.callsFor('prepareManagedSession')[0].input).toEqual({ agent_config_id: passed.id, cwd: '/repo' })
    await draft.close(); await controller.close()
  })
  it('keeps missing programs out of the picker after detection', async () => {
    const { controller, transport } = setup()
    transport.resolve('listAgentConfigs', []).resolve('listAvailableAgents', [catalog]).resolve('listAgentInstallJobs', [])
      .resolve('inspectAgentInstallation', { ...inspection, source: 'missing', command: null })
    controller.start(); await controller.refreshChoices(true); await flush()
    expect(get(controller).choices).toEqual([])
    for (const command of ['checkAgentConfig', 'installAgent', 'prepareManagedSession'] as const) expect(transport.callsFor(command)).toHaveLength(0)
    await controller.close()
  })
  it('retains explicit detection finishing after its draft closes', async () => {
    const { controller, transport } = setup()
    const pending = deferred<typeof connected>()
    transport.resolve('listAvailableAgents', []).resolve('listAgentInstallJobs', []).handle('checkAgentConfig', () => pending.promise)
    controller.start(); await flush()
    const scan = controller.refreshChoices(true); await flush()
    await controller.close(); pending.resolve(connected); await scan
    expect(readAgentDetectionCache(transport).connections[config.id]?.result.ok).toBe(true)
  })
  it('re-reads invalidations arriving during an older configuration read', async () => {
    const { controller, transport } = setup()
    const oldRead = deferred<AgentConfig[]>()
    const updated = { ...config, updated_at: 'later', env: { MODEL: 'new' } }
    transport.handle('listAgentConfigs', () => oldRead.promise)
    controller.start(); await flush()
    transport.resolve('listAgentConfigs', [updated])
    rememberAgentConnection(transport, updated, connected)
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'invalidate', runtime_generation: 'runtime', revision: '2', resources: [{ kind: 'agent_configurations' }] })
    oldRead.resolve([config]); await flush()
    expect(get(controller).choices[0]?.config).toEqual(updated)
    expect(readAgentDetectionCache(transport).connections[config.id]?.result.ok).toBe(true)
    await controller.close()
  })
  it('offers one canonical agent while preserving its additional profiles in settings', () => {
    const linked = { ...config, catalog_id: 'pi' }, unchecked = { ...config, catalog_id: 'pi', id: 'old' }, extra = { ...config, catalog_id: 'pi', id: 'other' }
    expect(draftAgentChoices([unchecked, linked, extra], [catalog], [inspection], checks(linked, extra)).map(choice => choice.key)).toEqual(['config:config'])
    expect(agentListItems([catalog], [unchecked, linked, extra], checks(linked, extra))[0].configs).toEqual([unchecked, linked, extra])
  })
  it('matches detection names for old DeepSeek ACP and duplicate Claude Code configurations', () => {
    const claude = { ...catalog, id: 'claude-acp', host_id: 'claude', name: 'Claude Code' }
    const deepseek = { ...catalog, id: 'deepseek-acp', host_id: 'dsh', name: 'DeepSeek (DSH)' }
    const original = { ...config, id: 'claude', catalog_id: claude.id, name: 'Claude Code', host_id: 'claude' }
    const duplicate = { ...original, id: 'claude-2', name: 'Claude Code (2)' }
    const renamed = { ...config, id: 'deepseek', catalog_id: deepseek.id, name: 'DeepSeek ACP', host_id: 'dsh' }
    const retired = { ...renamed, id: 'old-dsh', catalog_id: 'dsh', name: 'DeepSeek Harness' }
    const configs = [original, duplicate, renamed, retired]
    const connections = checks(duplicate, renamed, retired)
    const choices = draftAgentChoices(configs, [claude, deepseek], [], connections)
    expect(choices.map(choice => [choice.name, choice.config?.id])).toEqual([['Claude Code', duplicate.id], ['DeepSeek (DSH)', renamed.id]])
    const settings = agentListItems([claude, deepseek], configs, connections).filter(row => row.entry)
    expect(settings.map(row => [row.name, row.config?.id])).toEqual(choices.map(choice => [choice.name, choice.config?.id]))
  })
  it('keeps the explicitly selected profile in a restored draft without making another agent row', () => {
    const original = { ...config, catalog_id: 'pi' }
    const alternate = { ...original, id: 'second', name: 'Pi (2)', env: { ACCOUNT: 'other-account' } }
    const choices = draftAgentChoices([original, alternate], [catalog], [], checks(original, alternate), 'config:second')
    expect(choices).toHaveLength(1)
    expect(choices[0]).toMatchObject({ key: 'config:second', name: catalog.name, config: alternate })
  })
  it('switches checked launch profiles within one agent without changing its displayed name', async () => {
    const { controller, transport } = setup()
    const original = { ...config, catalog_id: 'pi' }
    const alternate = { ...original, id: 'second', name: 'Pi (2)', env: { ACCOUNT: 'other-account' } }
    transport.resolve('listAgentConfigs', [original, alternate]).resolve('listAvailableAgents', [catalog])
    rememberAgentConnection(transport, alternate, connected)
    controller.start(); await flush()
    expect(get(controller).choices[0].profiles).toEqual([original, alternate])
    controller.select('config:second', '/repo'); await flush()
    expect(get(controller).choices).toHaveLength(1)
    expect(get(controller).choices[0]).toMatchObject({ key: 'config:second', name: catalog.name, config: alternate })
    expect(transport.callsFor('prepareManagedSession').map(call => call.input.agent_config_id)).toEqual(['config', 'second'])
    expect(get(controller).text).toBe('Draft task')
    await controller.refreshChoices(); await flush()
    expect(get(controller).choice).toBe('config:second')
    await controller.close()
  })
  it('waits for catalog names without showing old aliases or forgetting the selected profile', async () => {
    const { controller, transport } = setup()
    const linked = { ...config, catalog_id: 'pi', name: 'Old Pi ACP' }
    const listing = deferred<AgentCatalogEntry[]>()
    transport.resolve('listAgentConfigs', [linked]).handle('listAvailableAgents', () => listing.promise)
    controller.start(); await flush()
    expect(get(controller)).toMatchObject({ choices: [], choice: 'config:config' })
    expect(transport.callsFor('prepareManagedSession')).toHaveLength(0)
    listing.resolve([catalog]); await flush()
    expect(get(controller)).toMatchObject({ choice: 'config:config', phase: 'ready' })
    expect(get(controller).choices[0].name).toBe(catalog.name)
    await controller.close()
  })
})

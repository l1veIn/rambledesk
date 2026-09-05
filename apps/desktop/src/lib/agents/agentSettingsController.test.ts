import { get } from 'svelte/store'
import { describe, expect, it, vi } from 'vitest'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
import type { AgentConfig } from '$lib/generated/feedback'
import { agentDraftInput, agentConfigDraft } from './agentConfigForm'
import { createAgentSettingsController } from './agentSettingsController'

const config: AgentConfig = {
  id: 'config-one', name: 'DeepSeek', host_id: 'dsh', protocol: 'acp', enabled: true,
  command: 'deepseek-acp', args: [], env: {}, created_at: 'old', updated_at: 'old',
}
async function flush() { await Promise.resolve(); await Promise.resolve(); await Promise.resolve() }

describe('Agent settings transport integration', () => {
  it('checks a legacy disabled profile directly while preserving its launch settings', async () => {
    const saved = { ...config, enabled: false, args: ['--account', 'work'], env: { TOKEN: 'keep' } }
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .resolve('listAgentConfigs', [saved])
      .handle('saveAgentConfig', input => ({ ...saved, ...input, id: saved.id }))
      .resolve('checkAgentConfig', { ok: true, message: 'Connected', details: [] })
    const controller = createAgentSettingsController(transport)
    controller.start(); await flush()
    await controller.check(saved.id)
    expect(transport.callsFor('saveAgentConfig')[0].input).toEqual({ ...saved, enabled: true })
    expect(transport.callsFor('checkAgentConfig')).toHaveLength(1)
    expect(get(controller).configs[0].enabled).toBe(true)
    controller.dispose()
  })
  it('resolves only an explicit catalog selection and ignores an older list response', async () => {
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true }).resolve('listAgentConfigs', [])
    const controller = createAgentSettingsController(transport)
    controller.start()
    await flush()
    expect(transport.callsFor('resolveCatalogAgent')).toHaveLength(0)
    let finish!: (configs: AgentConfig[]) => void
    transport.handle('listAgentConfigs', () => new Promise(resolve => { finish = resolve }))
    const oldRead = controller.refresh()
    await flush()
    const selected = { ...config, catalog_id: 'pi-acp', command: 'custom-wrapper', args: ['keep'], env: { TOKEN: 'keep' } }
    transport.resolve('resolveCatalogAgent', selected)
    const input = { agent_id: 'pi-acp', agent_config_id: selected.id, enable: true }
    expect(await controller.resolve(input)).toEqual(selected)
    finish([]); await oldRead
    expect(get(controller).configs).toEqual([selected])
    expect(transport.callsFor('saveAgentConfig')).toHaveLength(0)
    expect(transport.callsFor('resolveCatalogAgent')[0].input).toEqual(input)
    controller.dispose()
  })
  it('loads configs and refreshes only for configuration changes or a ready connection', async () => {
    const transport = new TestApplicationTransport(undefined).resolve('listAgentConfigs', [config])
    transport.markReady()
    const controller = createAgentSettingsController(transport)
    const dispose = controller.start()
    await flush()
    expect(get(controller).configs).toEqual([config])
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'invalidate', runtime_generation: 'runtime', revision: '1', resources: [{ kind: 'managed_session', session_id: 'another' }] })
    await flush()
    expect(transport.callsFor('listAgentConfigs')).toHaveLength(1)
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'invalidate', runtime_generation: 'runtime', revision: '2', resources: [{ kind: 'agent_configurations' }] })
    await flush()
    expect(transport.callsFor('listAgentConfigs')).toHaveLength(2)
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'ready', runtime_generation: 'runtime-new', revision: '0' })
    await flush()
    expect(transport.callsFor('listAgentConfigs')).toHaveLength(3)
    dispose()
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'invalidate', runtime_generation: 'runtime', revision: '3', resources: [{ kind: 'agent_configurations' }] })
    await flush()
    expect(transport.callsFor('listAgentConfigs')).toHaveLength(3)
  })

  it('does not let an older list response overwrite a just-saved config', async () => {
    const transport = new TestApplicationTransport(undefined).resolve('listAgentConfigs', [config])
    transport.markReady()
    const controller = createAgentSettingsController(transport)
    controller.start()
    await flush()
    let completeOldRead: ((configs: AgentConfig[]) => void) | undefined
    transport.handle('listAgentConfigs', () => new Promise<AgentConfig[]>((resolve) => { completeOldRead = resolve }))
    const oldRead = controller.refresh()
    await flush()
    const updated = { ...config, name: 'Edited', updated_at: 'new' }
    transport.resolve('saveAgentConfig', updated)
    await controller.save(agentDraftInput(agentConfigDraft(updated)))
    completeOldRead?.([config])
    await oldRead
    expect(get(controller).configs).toEqual([updated])
    controller.dispose()
  })

  it('leaving before readiness or before a list response prevents later state updates', async () => {
    const transport = new TestApplicationTransport(undefined).resolve('listAgentConfigs', [config])
    const controller = createAgentSettingsController(transport)
    const observed = vi.fn()
    const stopObserving = controller.subscribe(observed)
    controller.start()
    controller.dispose()
    observed.mockClear()
    transport.markReady()
    await flush()
    expect(transport.callsFor('listAgentConfigs')).toHaveLength(0)
    expect(observed).not.toHaveBeenCalled()
    stopObserving()

    let resolveRead: ((configs: AgentConfig[]) => void) | undefined
    transport.handle('listAgentConfigs', () => new Promise<AgentConfig[]>((resolve) => { resolveRead = resolve }))
    const next = createAgentSettingsController(transport)
    next.start()
    await flush()
    next.dispose()
    resolveRead?.([config])
    await flush()
    expect(get(next).configs).toEqual([])
  })

  it('keeps saved records after a failed mutation and forwards checked config identifiers', async () => {
    const transport = new TestApplicationTransport(undefined).resolve('listAgentConfigs', [config])
    transport.markReady()
    const controller = createAgentSettingsController(transport)
    controller.start()
    await flush()
    transport.reject('saveAgentConfig', new Error('Save failed'))
    await expect(controller.save(agentDraftInput(agentConfigDraft(config)))).rejects.toThrow('Save failed')
    expect(get(controller)).toMatchObject({ configs: [config], loading: false })
    const check = { ok: true, message: 'Connected', details: ['ACP ready'] }
    transport.resolve('checkAgentConfig', check)
    expect(await controller.check(config.id)).toEqual(check)
    expect(transport.callsFor('checkAgentConfig')[0].input).toEqual({ agent_config_id: config.id })
    transport.resolve('deleteAgentConfig', undefined)
    await controller.remove(config.id)
    expect(get(controller).configs).toEqual([])
    controller.dispose()
  })
})

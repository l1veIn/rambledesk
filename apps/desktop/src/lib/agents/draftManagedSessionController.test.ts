import { get } from 'svelte/store'
import { describe, expect, it, vi } from 'vitest'
import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import type { AgentCatalogEntry, AgentConfig, AgentInspection, ManagedSessionSnapshot } from '$lib/generated/feedback'
import { createDraftManagedSessionController, draftAgentChoices } from './draftManagedSessionController'
import { createManagedSessionDraftStorage } from './managedSessionDrafts'

const config: AgentConfig = { id: 'config', name: 'Pi', host_id: 'pi', protocol: 'acp', enabled: true, command: 'pi-acp', args: [], env: {}, created_at: '', updated_at: '' }
const catalog: AgentCatalogEntry = { id: 'pi', name: 'Pi', host_id: 'pi', description: '', connection_kind: 'bridge', distribution: { kind: 'npm', package: 'pi-acp', command: 'pi-acp', pinned_version: '1.0.0', node_required: '22.0.0' }, args: [], dependencies: [], verification: { status: 'unverified', versions: [], note: '' } }
const inspection: AgentInspection = { agent_id: 'pi', command: 'pi-acp', args: [], source: 'system', version: '1.0.0', checks: [], dependencies: [] }
function snapshot(id: string, lifecycle: 'prepared' | 'active' = 'prepared', connection: 'connected' | 'failed' = 'connected'): ManagedSessionSnapshot {
  return { session: { session_id: id, host_id: 'pi', host_session_id: id, title: 'Task', lifecycle,
    management: { kind: 'managed', protocol: 'acp', agent_config_id: 'config', cwd: '/repo', remote_session_id: `remote-${id}` }, created_at: '', updated_at: '' },
    runtime: { configuration: { options: [], models: null, modes: null }, connection, activity: 'idle', instance_id: 'runtime', config_updated_at: null,
      capabilities: { prompt: { image: false, audio: false, embedded_context: true, resource_links: true }, load_session: false, resume_session: false, http_mcp: false }, last_error: connection === 'failed' ? 'Connection failed' : null },
    activities: [], permissions: [], deliveries: [], deleting: false, recovery: null }
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
  it('keeps custom profiles distinct and matches catalog identity without launch heuristics', () => {
    const custom = { ...config, id: 'custom', enabled: false }
    const linked = { ...config, id: 'linked', catalog_id: 'pi', name: 'My Pi' }
    expect(draftAgentChoices([custom], [catalog], [inspection]).map((choice) => choice.key)).toEqual(['config:custom', 'catalog:pi'])
    expect(draftAgentChoices([custom, linked], [catalog], [inspection]).map((choice) => choice.key)).toEqual(['config:custom', 'config:linked'])
    expect(draftAgentChoices([], [catalog], [{ ...inspection, command: null }])).toEqual([])
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
    draft.start(); await flush()
    expect(get(draft).choices.map((choice) => choice.key)).toEqual(['catalog:pi'])
    expect(transport.callsFor('resolveCatalogAgent')).toHaveLength(0)
    draft.select('catalog:pi', '/repo'); await flush()
    expect(transport.callsFor('resolveCatalogAgent')[0].input).toEqual({ agent_id: 'pi', enable: true })
    expect(get(draft).choice).toBe('config:config')
    expect(get(draft).phase).toBe('ready')
    await draft.close()
    await controller.close()
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
    transport.resolve('sendManagedPromptContent', snapshot('one', 'active'))
    await controller.send('Draft task')
    expect(transport.callsFor('sendManagedPromptContent')[0].input).toEqual({ session_id: 'one', text: 'Draft task', content: [] })
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
    transport.handle('sendManagedPromptContent', () => sending.promise)
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
    transport.handle('sendManagedPromptContent', () => { throw new Error('Response lost') })
    transport.handle('getManagedSession', () => { throw new Error('Offline') })
    await expect(controller.send('Draft task')).rejects.toThrow('Response lost')
    expect(get(controller).awaitingAcknowledgement).toBe(true)
    controller.select('config:config', '/another')
    expect(get(controller).cwd).toBe('/repo')
    transport.resolve('getManagedSession', snapshot('one', 'active'))
    await controller.retry()
    expect(promoted).toHaveBeenCalledTimes(1)
    expect(transport.callsFor('sendManagedPromptContent')).toHaveLength(1)
    expect(transport.callsFor('startManagedSession')).toHaveLength(0)
    await controller.close()
    expect(transport.callsFor('discardPreparedSession')).toHaveLength(0)
  })

  it('restores a rejected first task and does not overwrite text typed during a later attempt', async () => {
    const { controller, transport } = setup()
    controller.start(); await flush()
    transport.handle('sendManagedPromptContent', () => { throw new Error('Not accepted') })
    await expect(controller.send('Draft task')).rejects.toThrow('Not accepted')
    expect(get(controller).text).toBe('Draft task')
    expect(get(controller).awaitingAcknowledgement).toBe(false)
    transport.resolve('startManagedSession', snapshot('one'))
    await controller.retry()
    const response = deferred<ManagedSessionSnapshot>()
    transport.handle('sendManagedPromptContent', () => response.promise)
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
    transport.handle('sendManagedPromptContent', () => response.promise)
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

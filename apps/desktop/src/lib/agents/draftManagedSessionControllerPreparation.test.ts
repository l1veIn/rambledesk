import { rememberAgentConnection } from './agentDetectionCache'
import { get } from 'svelte/store'
import { afterEach, describe, expect, it } from 'vitest'

import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import type { ManagedSessionSnapshot } from '$lib/generated/feedback'
import {
  captureDiagnostics,
  catalog,
  config,
  deferred,
  flush,
  releaseDiagnostics,
  setup,
  snapshot,
  stallDiagnostics,
} from './draftManagedSessionControllerTestHarness'

afterEach(() => {
  releaseDiagnostics()
})

describe('managed draft lifecycle: preparation, restore, diagnostics', () => {
  it('redacts failure evidence when it enters draft state, before environment settings can change', async () => {
    const { controller, transport } = setup()
    const failed = snapshot('one', 'prepared', 'failed')
    failed.runtime.last_error = 'Rejected old-secret-value'
    failed.runtime.failure = { stage: 'session', reason: 'authentication', message: 'Rejected old-secret-value' }
    rememberAgentConnection(transport, { ...config, env: { API_KEY: 'old-secret-value' } }, { ok: true, message: 'Connected', details: [] })
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
    rememberAgentConnection(transport, { ...config, id: 'another', name: 'Another agent' }, { ok: true, message: 'Connected', details: [] })
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
    transport.resolve('listAvailableAgents', [{ ...catalog, id: catalogId }])
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
    stallDiagnostics()
    const fresh = setup()
    fresh.controller.start(); await flush()
    expect(get(fresh.controller).phase).toBe('ready')
    await fresh.controller.close()
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

  it('discards an unsent connection and requires a new check after its profile changes', async () => {
    const { controller, transport } = setup()
    controller.start(); await flush()
    transport.resolve('listAgentConfigs', [{ ...config, updated_at: 'later', env: { MODEL: 'new' } }]).resolve('prepareManagedSession', snapshot('two'))
    await controller.refreshChoices(); await flush()
    expect(transport.callsFor('discardPreparedSession').map((call) => call.input)).toEqual([{ session_id: 'one' }])
    expect(get(controller)).toMatchObject({ snapshot: null, choices: [], choice: '', phase: 'idle' })
    expect(transport.callsFor('prepareManagedSession')).toHaveLength(1)
    expect(get(controller).text).toBe('Draft task')
    await controller.close()
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

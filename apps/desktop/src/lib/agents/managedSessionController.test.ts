import { get } from 'svelte/store'
import { describe, expect, it, vi } from 'vitest'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
import type { ManagedSessionSnapshot } from '$lib/generated/feedback'
import { createManagedSessionController } from './managedSessionController'

function snapshot(id: string, text = ''): ManagedSessionSnapshot {
  return {
    session: { session_id: id, host_id: 'dsh', host_session_id: `host-${id}`, title: id,
      management: { kind: 'managed', protocol: 'acp', agent_config_id: 'config', cwd: '/repo', remote_session_id: `remote-${id}` },
      created_at: '2026-09-04', updated_at: '2026-09-04' },
    runtime: { configuration: { options: [], modes: null, models: null }, connection: 'connected', activity: 'idle', instance_id: `instance-${id}`, config_updated_at: null,
      capabilities: { prompt: { image: false, audio: false, embedded_context: false, resource_links: true }, load_session: true, resume_session: false, http_mcp: true }, last_error: null },
    activities: text ? [{ id: 'message', session_id: id, sequence: 1, turn_id: 'turn',
      kind: 'agent_message', text, tool_call_id: null, created_at: '2026-09-04' }] : [],
    permissions: [],
    deliveries: [],
    deleting: false,
    recovery: null,
  }
}

function historySnapshot(sequences: number[], text = 'old'): ManagedSessionSnapshot {
  const view = snapshot('history', text)
  view.activities = sequences.map(sequence => ({ ...view.activities[0], id: `row-${sequence}`, sequence, text }))
  return view
}

describe('managed session history', () => {
  it('retains older pages while newer live rows replace overlapping activity', async () => {
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true }).resolve('getManagedSession', historySnapshot([1001, 1002]))
    const controller = createManagedSessionController(transport, 'history')
    controller.start()
    await flush()
    transport.resolve('listManagedSessionActivity', { activities: historySnapshot([999, 1000]).activities, has_more: true })
    await controller.loadOlder()
    expect(transport.callsFor('listManagedSessionActivity')[0].input).toEqual({ session_id: 'history', before_sequence: 1001, limit: 100 })
    expect(get(controller).snapshot?.activities.map(row => row.sequence)).toEqual([999, 1000, 1001, 1002])
    transport.resolve('getManagedSession', historySnapshot([1002, 1003], 'new'))
    invalidate(transport, 'history')
    await flush()
    expect(get(controller).snapshot?.activities.map(row => row.sequence)).toEqual([999, 1000, 1001, 1002, 1003])
    expect(get(controller).snapshot?.activities.find(row => row.sequence === 1002)?.text).toBe('new')
    controller.dispose()
  })

  it('rejects foreign pages and ignores a page finishing after the workspace closes', async () => {
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true }).resolve('getManagedSession', historySnapshot([10]))
    const controller = createManagedSessionController(transport, 'history')
    controller.start()
    await flush()
    transport.resolve('listManagedSessionActivity', { activities: snapshot('foreign', 'secret').activities, has_more: false })
    await controller.loadOlder()
    expect(get(controller).historyError).toContain('invalid session')
    expect(get(controller).snapshot?.activities).toHaveLength(1)
    const pending = deferred<{ activities: ManagedSessionSnapshot['activities']; has_more: boolean }>()
    transport.handle('listManagedSessionActivity', () => pending.promise)
    const loading = controller.loadOlder()
    controller.dispose()
    pending.resolve({ activities: historySnapshot([1, 2]).activities, has_more: false })
    await loading
    expect(get(controller).snapshot?.activities).toHaveLength(1)
  })
})

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => { resolve = done })
  return { promise, resolve }
}
async function flush() { for (let i = 0; i < 12; i += 1) await Promise.resolve() }
function invalidate(transport: TestApplicationTransport, id: string) {
  transport.emit(APPLICATION_EVENTS_STREAM, { type: 'invalidate', runtime_generation: 'runtime', revision: '1', resources: [{ kind: 'managed_session', session_id: id }] })
}

describe('managed workspace transport integration', () => {
  it('does not dispatch work from a durable deleting snapshot', async () => {
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true }).resolve('getManagedSession', { ...snapshot('one'), deleting: true })
    const controller = createManagedSessionController(transport, 'one')
    controller.start()
    await flush()
    for (const action of [controller.startAgent, controller.stopAgent, controller.cancel,
      () => controller.prompt('New work'), () => controller.respondPermission('permission', 'allow'),
      () => controller.resolveDelivery('feedback', 'retry')]) {
      await expect(action()).rejects.toThrow('being deleted')
    }
    expect(transport.calls.map((call) => call.name)).toEqual(['getManagedSession'])
    controller.dispose()
  })
  it('isolates two sessions and refreshes only their own invalidations or transport readiness', async () => {
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .handle('getManagedSession', ({ session_id }) => snapshot(session_id))
    const first = createManagedSessionController(transport, 'one')
    const second = createManagedSessionController(transport, 'two')
    first.start(); second.start()
    await flush()
    expect(get(first).snapshot?.session.session_id).toBe('one')
    expect(get(second).snapshot?.session.session_id).toBe('two')
    invalidate(transport, 'one')
    await flush()
    expect(transport.callsFor('getManagedSession').map((call) => call.input.session_id)).toEqual(['one', 'two', 'one'])
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'invalidate', runtime_generation: 'runtime', revision: '2', resources: [{ kind: 'navigation' }, { kind: 'agent_configurations' }] })
    await flush()
    expect(transport.callsFor('getManagedSession')).toHaveLength(3)
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'ready', runtime_generation: 'reconnected', revision: '0' })
    await flush()
    expect(transport.callsFor('getManagedSession').map((call) => call.input.session_id)).toEqual(['one', 'two', 'one', 'one', 'two'])
    first.dispose(); second.dispose()
    invalidate(transport, 'one')
    await flush()
    expect(transport.callsFor('getManagedSession')).toHaveLength(5)
    expect(transport.callsFor('stopManagedSession')).toHaveLength(0)
    expect(transport.callsFor('listHostSessions')).toHaveLength(0)
  })

  it('coalesces activity bursts behind one in-flight read without starving live updates', async () => {
    const initial = deferred<ManagedSessionSnapshot>()
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .handle('getManagedSession', () => initial.promise)
    const controller = createManagedSessionController(transport, 'one')
    controller.start()
    await flush()
    for (let i = 0; i < 25; i += 1) invalidate(transport, 'one')
    await flush()
    expect(transport.callsFor('getManagedSession')).toHaveLength(1)
    transport.resolve('getManagedSession', snapshot('one', 'latest streamed output'))
    initial.resolve(snapshot('one', 'earlier output'))
    await flush()
    expect(transport.callsFor('getManagedSession')).toHaveLength(2)
    expect(get(controller).snapshot?.activities[0].text).toBe('latest streamed output')
    controller.dispose()
  })

  it('does not let a late prompt response overwrite newer activity or another selected session', async () => {
    const turn = deferred<ManagedSessionSnapshot>()
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .handle('getManagedSession', ({ session_id }) => snapshot(session_id, 'current'))
      .handle('sendManagedPrompt', () => turn.promise)
    const first = createManagedSessionController(transport, 'one')
    const second = createManagedSessionController(transport, 'two')
    first.start(); second.start()
    await flush()
    const sending = first.prompt('Work on project one')
    transport.handle('getManagedSession', ({ session_id }) => snapshot(session_id, 'newer'))
    invalidate(transport, 'one')
    await flush()
    const observed = vi.fn()
    const unsubscribe = first.subscribe(observed)
    first.dispose()
    observed.mockClear()
    turn.resolve(snapshot('one', 'stale command response'))
    await sending
    await flush()
    expect(observed).not.toHaveBeenCalled()
    expect(get(first).snapshot?.activities[0].text).toBe('newer')
    expect(get(second).snapshot?.activities[0].text).toBe('current')
    expect(transport.callsFor('sendManagedPrompt')[0].input).toEqual({ session_id: 'one', text: 'Work on project one' })
    expect(transport.callsFor('sendManagedPrompt')).toHaveLength(1)
    unsubscribe(); second.dispose()
  })

  it('forwards lifecycle and permission actions to the owning session without replaying failures', async () => {
    const current = snapshot('one')
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .resolve('getManagedSession', current)
      .resolve('startManagedSession', current)
      .resolve('stopManagedSession', current)
      .resolve('cancelManagedPrompt', current)
      .resolve('respondManagedPermission', current)
    const controller = createManagedSessionController(transport, 'one')
    controller.start()
    await flush()
    await controller.startAgent()
    await controller.cancel()
    await controller.respondPermission('request-one', 'allow-once')
    await controller.respondPermission('request-two', null)
    await controller.stopAgent()
    expect(transport.callsFor('respondManagedPermission').map((call) => call.input)).toEqual([
      { session_id: 'one', request_id: 'request-one', option_id: 'allow-once' },
      { session_id: 'one', request_id: 'request-two', option_id: null },
    ])
    expect(transport.callsFor('startManagedSession')).toHaveLength(0)
    for (const name of ['cancelManagedPrompt', 'stopManagedSession'] as const) {
      expect(transport.callsFor(name).map((call) => call.input)).toEqual([{ session_id: 'one' }])
    }
    transport.reject('sendManagedPrompt', new Error('Connection ended'))
    await expect(controller.prompt('Keep my draft')).rejects.toThrow('Connection ended')
    expect(transport.callsFor('sendManagedPrompt')).toHaveLength(1)
    controller.dispose()
    await expect(controller.startAgent()).rejects.toThrow('no longer open')
    expect(transport.callsFor('startManagedSession')).toHaveLength(0)
  })

  it('rejects foreign snapshots and disposes pending readiness and requests without runtime commands', async () => {
    const transport = new TestApplicationTransport(undefined)
      .resolve('getManagedSession', snapshot('foreign'))
    const beforeReady = createManagedSessionController(transport, 'one')
    beforeReady.start()
    beforeReady.dispose()
    transport.markReady()
    await flush()
    expect(transport.calls).toHaveLength(0)
    const controller = createManagedSessionController(transport, 'one')
    controller.start()
    await flush()
    expect(get(controller)).toMatchObject({ snapshot: null, loading: false, error: 'The agent returned a different session.' })
    const pending = deferred<ManagedSessionSnapshot>()
    transport.handle('getManagedSession', () => pending.promise)
    controller.refresh()
    await flush()
    controller.dispose()
    pending.resolve(snapshot('one'))
    await flush()
    expect(get(controller).snapshot).toBeNull()
    expect(transport.callsFor('stopManagedSession')).toHaveLength(0)
  })

  it('resolves uncertain feedback only through the requested action and reads the resulting projection', async () => {
    const current = snapshot('one')
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .resolve('getManagedSession', current)
      .resolve('resolveFeedbackDelivery', current)
    const controller = createManagedSessionController(transport, 'one')
    controller.start()
    await flush()
    expect(transport.callsFor('resolveFeedbackDelivery')).toHaveLength(0)
    await controller.resolveDelivery('feedback-one', 'retry')
    await controller.resolveDelivery('feedback-two', 'acknowledge')
    expect(transport.callsFor('resolveFeedbackDelivery').map((call) => call.input)).toEqual([
      { session_id: 'one', request_id: 'feedback-one', action: 'retry' },
      { session_id: 'one', request_id: 'feedback-two', action: 'acknowledge' },
    ])
    transport.reject('resolveFeedbackDelivery', new Error('Outcome unknown'))
    await expect(controller.resolveDelivery('feedback-three', 'retry')).rejects.toThrow('Outcome unknown')
    expect(transport.callsFor('resolveFeedbackDelivery')).toHaveLength(3)
    controller.dispose()
  })
})

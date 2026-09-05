import { get } from 'svelte/store'
import { describe, expect, it, vi } from 'vitest'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
import type { ManagedSessionSnapshot, SessionConnectionState } from '$lib/generated/feedback'
import { createManagedSessionController } from './managedSessionController'

function snapshot(connection: SessionConnectionState = 'stopped'): ManagedSessionSnapshot {
  return {
    session: { session_id: 'one', host_id: 'fixture', host_session_id: 'one', title: 'Existing session',
      management: { kind: 'managed', protocol: 'acp', agent_config_id: 'config', cwd: '/repo', remote_session_id: 'original' },
      created_at: '2026-09-05', updated_at: '2026-09-05' },
    runtime: { connection, activity: 'idle', instance_id: connection === 'connected' ? 'instance' : null,
      capabilities: { load_session: true, resume_session: false, http_mcp: false,
        prompt: { image: false, audio: false, embedded_context: false, resource_links: false } },
      configuration: { options: [], modes: null, models: null }, config_updated_at: null, last_error: null },
    activities: [], permissions: [], deliveries: [], deleting: false, recovery: null,
  }
}
function recovery(status: 'interrupted' | 'unclosed'): NonNullable<ManagedSessionSnapshot['recovery']> {
  return { session_id: 'one', status, run_id: 'previous', active_turn_id: null, interrupted_turn_id: 'unfinished', last_error: null, updated_at: '2026-09-05' }
}
function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (cause: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}
async function flush() { for (let i = 0; i < 32; i += 1) await Promise.resolve() }
function invalidate(transport: TestApplicationTransport) {
  transport.emit(APPLICATION_EVENTS_STREAM, { type: 'invalidate', runtime_generation: 'runtime', revision: '1', resources: [{ kind: 'managed_session', session_id: 'one' }] })
}
function ready(transport: TestApplicationTransport, runtime_generation = 'runtime') {
  transport.emit(APPLICATION_EVENTS_STREAM, { type: 'ready', runtime_generation, revision: '0' })
}

describe('opening an Agent workspace', () => {
  it('connects a stopped session once and only reads the current projection afterward', async () => {
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .resolve('getManagedSession', snapshot())
      .handle('startManagedSession', () => {
        transport.resolve('getManagedSession', snapshot('connected'))
        return snapshot('connected')
      })
    const controller = createManagedSessionController(transport, 'one')
    controller.start()
    await flush()
    expect(get(controller)).toMatchObject({ connecting: false, connectionError: '', snapshot: { runtime: { connection: 'connected' } } })
    expect(transport.callsFor('startManagedSession')).toEqual([{ name: 'startManagedSession', input: { session_id: 'one' } }])
    expect(transport.calls.every(call => ['getManagedSession', 'startManagedSession'].includes(call.name))).toBe(true)
    controller.dispose()
    expect(transport.callsFor('stopManagedSession')).toHaveLength(0)
  })

  it.each(['connected', 'connecting'] as const)('does not start an already %s runtime, including explicit retry', async connection => {
    const current = snapshot(connection)
    current.recovery = recovery('unclosed')
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true }).resolve('getManagedSession', current)
    const controller = createManagedSessionController(transport, 'one')
    controller.start()
    await flush()
    await controller.startAgent()
    invalidate(transport); ready(transport)
    await flush()
    expect(transport.callsFor('startManagedSession')).toHaveLength(0)
    expect(get(controller).connectionError).toBe('')
    controller.dispose()
  })

  it('shares an in-flight start across refresh bursts, explicit retry, and a rapid tab remount', async () => {
    const pending = deferred<ManagedSessionSnapshot>()
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .resolve('getManagedSession', snapshot())
      .handle('startManagedSession', () => pending.promise)
    const first = createManagedSessionController(transport, 'one')
    first.start()
    await flush()
    expect(get(first).connecting).toBe(true)
    for (let i = 0; i < 20; i += 1) invalidate(transport)
    ready(transport)
    const retry = first.startAgent()
    await flush()
    const observed = vi.fn()
    const unsubscribe = first.subscribe(observed)
    first.dispose()
    observed.mockClear()
    const second = createManagedSessionController(transport, 'one')
    second.start()
    await flush()
    expect(transport.callsFor('startManagedSession')).toHaveLength(1)
    transport.resolve('getManagedSession', snapshot('connected'))
    pending.resolve(snapshot('connected'))
    await retry
    await flush()
    expect(observed).not.toHaveBeenCalled()
    expect(get(second).snapshot?.runtime.connection).toBe('connected')
    expect(transport.callsFor('stopManagedSession')).toHaveLength(0)
    unsubscribe(); second.dispose()
  })

  it('keeps a failed attempt visible without retries from reads or reconnect events; explicit retry can succeed', async () => {
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .resolve('getManagedSession', snapshot())
      .reject('startManagedSession', new Error('Connection unavailable'))
    const controller = createManagedSessionController(transport, 'one')
    controller.start()
    await flush()
    expect(get(controller)).toMatchObject({ connecting: false, connectionError: 'Connection unavailable' })
    for (let i = 0; i < 5; i += 1) { ready(transport, `runtime-${i}`); invalidate(transport); await flush() }
    expect(transport.callsFor('startManagedSession')).toHaveLength(1)
    expect(get(controller).connectionError).toBe('Connection unavailable')
    transport.handle('startManagedSession', () => {
      transport.resolve('getManagedSession', snapshot('connected'))
      return snapshot('connected')
    })
    await controller.startAgent()
    await flush()
    expect(transport.callsFor('startManagedSession')).toHaveLength(2)
    expect(get(controller).connectionError).toBe('')
    controller.dispose()
  })

  it('recovers an initially live connection once, then offers Retry if that connection ends again', async () => {
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .resolve('getManagedSession', snapshot('connected'))
      .handle('startManagedSession', () => {
        transport.resolve('getManagedSession', snapshot('connected'))
        return snapshot('connected')
      })
    const controller = createManagedSessionController(transport, 'one')
    controller.start()
    await flush()
    transport.resolve('getManagedSession', snapshot('disconnected'))
    invalidate(transport)
    await flush()
    expect(transport.callsFor('startManagedSession')).toHaveLength(1)
    expect(get(controller).snapshot?.runtime.connection).toBe('connected')
    transport.resolve('getManagedSession', snapshot('disconnected'))
    invalidate(transport)
    await flush()
    expect(transport.callsFor('startManagedSession')).toHaveLength(1)
    expect(get(controller).connectionError).toBe('Agent connection ended. Retry to reconnect.')
    controller.dispose()
  })

  it.each(['success', 'failure'] as const)('does not let an old runtime connection %s overwrite the current runtime error', async outcome => {
    const pending = deferred<ManagedSessionSnapshot>()
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .resolve('getManagedSession', snapshot())
      .handle('startManagedSession', () => pending.promise)
    const controller = createManagedSessionController(transport, 'one')
    controller.start(); ready(transport, 'old')
    await flush()
    const failed = snapshot('failed'); failed.runtime.last_error = 'Current runtime needs attention'
    transport.resolve('getManagedSession', failed)
    ready(transport, 'new')
    await flush()
    expect(get(controller).connectionError).toBe('Current runtime needs attention')
    if (outcome === 'success') pending.resolve(snapshot('connected'))
    else pending.reject(new Error('Old runtime failed'))
    await flush()
    expect(get(controller)).toMatchObject({ connecting: false, connectionError: 'Current runtime needs attention' })
    expect(transport.callsFor('startManagedSession')).toHaveLength(1)
    controller.dispose()
  })

  it('does not launch from deleting, failed, prepared, or unresolved recovery snapshots', async () => {
    const unclosed = snapshot(); unclosed.recovery = recovery('unclosed')
    const cleanup = snapshot('disconnected'); cleanup.runtime.instance_id = 'unclosed-instance'
    const missing = snapshot(); missing.recovery = recovery('interrupted')
    if (missing.session.management.kind === 'managed') missing.session.management.remote_session_id = null
    const prepared = snapshot(); prepared.session.lifecycle = 'prepared'
    for (const current of [{ ...snapshot(), deleting: true }, snapshot('failed'), unclosed, cleanup, missing, prepared]) {
      const transport = new TestApplicationTransport(undefined, { initiallyReady: true }).resolve('getManagedSession', current)
      const controller = createManagedSessionController(transport, 'one')
      controller.start()
      await flush()
      invalidate(transport)
      await flush()
      expect(transport.callsFor('startManagedSession')).toHaveLength(0)
      if (!current.deleting && current.session.lifecycle !== 'prepared') expect(get(controller).connectionError).not.toBe('')
      controller.dispose()
    }
  })

  it('defers connection while deletion is pending and resumes only after the caller refreshes', async () => {
    let blocked = true
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .resolve('getManagedSession', snapshot())
      .handle('startManagedSession', () => {
        transport.resolve('getManagedSession', snapshot('connected'))
        return snapshot('connected')
      })
    const controller = createManagedSessionController(transport, 'one', { autoConnectBlocked: () => blocked })
    controller.start()
    await flush()
    await expect(controller.startAgent()).rejects.toThrow('being deleted')
    expect(transport.callsFor('startManagedSession')).toHaveLength(0)
    blocked = false
    controller.refresh()
    await flush()
    expect(transport.callsFor('startManagedSession')).toHaveLength(1)
    controller.dispose()
  })

  it('can resume an interrupted original context without replaying its prompt or uncertain delivery', async () => {
    const current = snapshot('disconnected'); current.recovery = recovery('interrupted')
    current.deliveries = [{ request_id: 'feedback', session_id: 'one', resolution: 'feedback_submitted', state: 'uncertain',
      attempt_id: 'attempt', created_at: '2026-09-05', updated_at: '2026-09-05', last_error: 'Interrupted' }]
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .resolve('getManagedSession', current)
      .handle('startManagedSession', () => {
        transport.resolve('getManagedSession', { ...current, runtime: snapshot('connected').runtime })
        return snapshot('connected')
      })
    const controller = createManagedSessionController(transport, 'one')
    controller.start()
    await flush()
    expect(transport.callsFor('startManagedSession')).toHaveLength(1)
    expect(transport.calls.every(call => ['getManagedSession', 'startManagedSession'].includes(call.name))).toBe(true)
    expect(get(controller).snapshot?.deliveries[0].state).toBe('uncertain')
    controller.dispose()
  })

  it('permits one new connection after a runtime generation changes, but respects an explicit stop', async () => {
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .resolve('getManagedSession', snapshot())
      .handle('startManagedSession', () => {
        transport.resolve('getManagedSession', snapshot('connected'))
        return snapshot('connected')
      })
      .handle('stopManagedSession', () => {
        transport.resolve('getManagedSession', snapshot())
        return snapshot()
      })
    const controller = createManagedSessionController(transport, 'one')
    controller.start(); ready(transport, 'first')
    await flush()
    transport.resolve('getManagedSession', snapshot())
    ready(transport, 'second')
    await flush()
    expect(transport.callsFor('startManagedSession')).toHaveLength(2)
    await controller.stopAgent()
    await flush()
    ready(transport, 'third')
    await flush()
    expect(transport.callsFor('startManagedSession')).toHaveLength(2)
    controller.dispose()
  })

  it('does not start from an old read invalidated by transport readiness or after view disposal', async () => {
    const old = deferred<ManagedSessionSnapshot>()
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .handle('getManagedSession', () => old.promise)
    const controller = createManagedSessionController(transport, 'one')
    controller.start()
    await flush()
    transport.resolve('getManagedSession', snapshot('connected'))
    ready(transport)
    old.resolve(snapshot())
    await flush()
    expect(transport.callsFor('startManagedSession')).toHaveLength(0)
    const late = deferred<ManagedSessionSnapshot>()
    transport.handle('getManagedSession', () => late.promise)
    controller.refresh()
    await flush()
    controller.dispose()
    late.resolve(snapshot())
    await flush()
    expect(transport.callsFor('startManagedSession')).toHaveLength(0)
    expect(transport.callsFor('stopManagedSession')).toHaveLength(0)
  })
})

import { get } from 'svelte/store'
import { describe, expect, it } from 'vitest'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
import type { ApplicationResourceKey, FeedbackDelivery, ManagedFeedbackStatus, ManagedSessionSnapshot } from '$lib/generated/feedback'
import { createManagedFeedbackStatusController } from './managedFeedbackStatusController'

function delivery(requestId = 'request', sessionId = 'one'): FeedbackDelivery {
  return { request_id: requestId, session_id: sessionId, resolution: 'feedback_submitted',
    state: 'uncertain', attempt_id: 'attempt', created_at: '2026-09-05', updated_at: '2026-09-05', last_error: 'Interrupted' }
}
function status(sessionId = 'one', deliveries: FeedbackDelivery[] = []): ManagedFeedbackStatus {
  return { session_id: sessionId, deleting: false, deliveries }
}
async function flush() { for (let i = 0; i < 24; i += 1) await Promise.resolve() }
function invalidate(transport: TestApplicationTransport, resources: ApplicationResourceKey[]) {
  transport.emit(APPLICATION_EVENTS_STREAM, { type: 'invalidate', runtime_generation: 'runtime', revision: '1', resources })
}
function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(done => { resolve = done })
  return { promise, resolve }
}

describe('request-side managed feedback status', () => {
  it('reads only lightweight status and refreshes relevant request or session changes', async () => {
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .handle('getManagedFeedbackStatus', ({ session_id }) => status(session_id))
    const controller = createManagedFeedbackStatusController(transport, 'one', 'request')
    controller.start()
    await flush()
    invalidate(transport, [{ kind: 'managed_session', session_id: 'two' }, { kind: 'published_feedback', request_id: 'other' }, { kind: 'agent_configurations' }])
    await flush()
    expect(transport.calls).toHaveLength(1)
    invalidate(transport, [{ kind: 'published_feedback', request_id: 'request' }])
    invalidate(transport, [{ kind: 'managed_session', session_id: 'one' }])
    await flush()
    expect(transport.calls.map(call => call.name)).toEqual(['getManagedFeedbackStatus', 'getManagedFeedbackStatus'])
    expect(get(controller).status).toEqual(status())
    controller.dispose()
    invalidate(transport, [{ kind: 'all' }])
    await flush()
    expect(transport.calls).toHaveLength(2)
  })

  it('resolves only this request once and refreshes status without using a full mutation snapshot', async () => {
    const mutation = deferred<ManagedSessionSnapshot>()
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .resolve('getManagedFeedbackStatus', status('one', [delivery(), delivery('other')]))
      .handle('resolveFeedbackDelivery', () => mutation.promise)
    const controller = createManagedFeedbackStatusController(transport, 'one', 'request')
    controller.start()
    await flush()
    const action = controller.resolve('retry')
    await controller.resolve('acknowledge')
    expect(transport.callsFor('resolveFeedbackDelivery')).toEqual([{ name: 'resolveFeedbackDelivery', input: { session_id: 'one', request_id: 'request', action: 'retry' } }])
    transport.resolve('getManagedFeedbackStatus', status('one', [{ ...delivery(), state: 'pending' }]))
    mutation.resolve({} as ManagedSessionSnapshot)
    await action
    await flush()
    expect(get(controller).status?.deliveries[0].state).toBe('pending')
    await controller.resolve('acknowledge')
    expect(transport.callsFor('resolveFeedbackDelivery')).toHaveLength(1)
    controller.dispose()
  })

  it('blocks delivery changes while deleting, and preserves the delete lock after the row disappears', async () => {
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .resolve('getManagedFeedbackStatus', { ...status('one', [delivery()]), deleting: true })
    const controller = createManagedFeedbackStatusController(transport, 'one', 'request')
    controller.start()
    await flush()
    await controller.resolve('retry')
    expect(transport.callsFor('resolveFeedbackDelivery')).toHaveLength(0)
    transport.reject('getManagedFeedbackStatus', { code: 'MANAGED_SESSION_NOT_FOUND' })
    controller.refresh()
    await flush()
    expect(get(controller).status?.deleting).toBe(true)
    expect(get(controller).status?.deliveries).toEqual([delivery()])
    expect(get(controller).error).toContain('Could not load')
    controller.dispose()
  })

  it('ignores old-runtime and disposed reads, and rejects another session projection', async () => {
    const old = deferred<ManagedFeedbackStatus>()
    const fresh = deferred<ManagedFeedbackStatus>()
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .handle('getManagedFeedbackStatus', () => old.promise)
    const controller = createManagedFeedbackStatusController(transport, 'one', 'request')
    controller.start()
    await flush()
    transport.handle('getManagedFeedbackStatus', () => fresh.promise)
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'ready', runtime_generation: 'new', revision: '0' })
    old.resolve({ ...status(), deleting: true })
    await flush()
    expect(get(controller).status).toBeNull()
    fresh.resolve(status('foreign'))
    await flush()
    expect(get(controller).status).toBeNull()
    expect(get(controller).error).toContain('Could not load')
    const final = deferred<ManagedFeedbackStatus>()
    transport.handle('getManagedFeedbackStatus', () => final.promise)
    controller.refresh()
    await flush()
    controller.dispose()
    final.resolve(status())
    await flush()
    expect(get(controller).status).toBeNull()
  })

  it('keeps a failed explicit decision visible and never replays it', async () => {
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .resolve('getManagedFeedbackStatus', status('one', [delivery()]))
      .reject('resolveFeedbackDelivery', new Error('secret from adapter'))
    const controller = createManagedFeedbackStatusController(transport, 'one', 'request')
    controller.start()
    await flush()
    await controller.resolve('retry')
    await flush()
    expect(get(controller).resolveError).toBe('Could not update feedback continuation.')
    expect(get(controller).status?.deliveries[0].state).toBe('uncertain')
    expect(transport.callsFor('resolveFeedbackDelivery')).toHaveLength(1)
    controller.dispose()
  })
})

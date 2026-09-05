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
  it('makes a failed completion repair retryable even when the entire history is already loaded', async () => {
    const all = historySnapshot(Array.from({ length: 200 }, (_, index) => index + 1)).activities
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .resolve('getManagedSession', { ...snapshot('history'), runtime: { ...snapshot('history').runtime, activity: 'running' as const }, activities: all.slice(0, 100) })
      .reject('listManagedSessionActivity', new Error('History query failed'))
    const controller = createManagedSessionController(transport, 'history')
    controller.start(); await flush()
    transport.resolve('getManagedSession', { ...snapshot('history'), activities: all.slice(100) })
    invalidate(transport, 'history'); await flush()
    expect(get(controller)).toMatchObject({ historyHasMore: false, historyError: 'History query failed' })
    invalidate(transport, 'history'); await flush()
    expect(transport.callsFor('listManagedSessionActivity')).toHaveLength(1)
    transport.resolve('listManagedSessionActivity', { activities: all.slice(0, 100).map(row => ({ ...row, text: 'Final tool state' })), has_more: false })
    await controller.loadOlder()
    expect(get(controller)).toMatchObject({ historyHasMore: false, historyError: '', historyLoading: false })
    expect(get(controller).snapshot?.activities[0].text).toBe('Final tool state')
    controller.dispose()
  })

  it('repairs loaded early tools at completion, paging only that turn and never repeating for idle invalidations', async () => {
    const all = historySnapshot(Array.from({ length: 300 }, (_, index) => index + 1)).activities
      .map(row => ({ ...row, turn_id: row.sequence <= 20 ? 'older-turn' : 'working-turn' }))
    const initial = { ...snapshot('history'), runtime: { ...snapshot('history').runtime, activity: 'running' as const }, activities: all.slice(100, 200) }
    const completed = { ...snapshot('history'), activities: all.slice(200) }
    completed.activities[99] = { ...completed.activities[99], kind: 'status', text: 'Turn finished: EndTurn' }
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .resolve('getManagedSession', initial)
      .resolve('listManagedSessionActivity', { activities: all.slice(0, 100), has_more: false })
    const controller = createManagedSessionController(transport, 'history')
    controller.start(); await flush()
    const untouched = get(controller).snapshot!.activities[0]
    transport.handle('listManagedSessionActivity', ({ before_sequence }) => {
      const first = Math.max(20, before_sequence - 61)
      return { activities: all.slice(first, before_sequence - 1).map(row => ({ ...row, text: `Final tool ${row.sequence}` })), has_more: first > 0 }
    })
    transport.resolve('getManagedSession', completed)
    invalidate(transport, 'history'); await flush(); await flush()
    expect(get(controller).snapshot?.activities.find(row => row.sequence === 21)?.text).toBe('Final tool 21')
    expect(get(controller).snapshot?.activities.find(row => row.sequence === 200)?.text).toBe('Final tool 200')
    expect(get(controller).snapshot?.activities[0]).toBe(untouched)
    const repairs = transport.callsFor('listManagedSessionActivity').slice(1)
    expect(repairs.map(call => call.input.before_sequence)).toEqual([201, 141, 81])
    expect(repairs.every(call => call.input.turn_limit === 1 && call.input.limit === 1000)).toBe(true)
    for (let index = 0; index < 3; index += 1) { invalidate(transport, 'history'); await flush() }
    expect(transport.callsFor('listManagedSessionActivity')).toHaveLength(4)
    controller.dispose()
  })

  it('ignores a late repair after a newer live row, runtime generation change, or disposal', async () => {
    for (const outcome of ['newer-live', 'new-generation', 'disposed'] as const) {
      const all = historySnapshot(Array.from({ length: 200 }, (_, index) => index + 1)).activities
      const initial = { ...snapshot('history'), runtime: { ...snapshot('history').runtime, activity: 'running' as const }, activities: all.slice(0, 100) }
      const completed = { ...snapshot('history'), activities: all.slice(100) }
      const pending = deferred<{ activities: ManagedSessionSnapshot['activities']; has_more: boolean }>()
      const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
        .resolve('getManagedSession', initial)
        .handle('listManagedSessionActivity', () => pending.promise)
      const controller = createManagedSessionController(transport, 'history')
      controller.start(); await flush()
      transport.emit(APPLICATION_EVENTS_STREAM, { type: 'ready', runtime_generation: 'first', revision: '0' }); await flush()
      transport.resolve('getManagedSession', completed)
      invalidate(transport, 'history'); await flush()
      expect(transport.callsFor('listManagedSessionActivity')).toHaveLength(1)
      if (outcome === 'newer-live') {
        transport.resolve('getManagedSession', { ...completed, activities: all.map(row => row.sequence === 50 ? { ...row, text: 'Newer live tool' } : row) })
        invalidate(transport, 'history'); await flush()
      } else if (outcome === 'new-generation') {
        transport.emit(APPLICATION_EVENTS_STREAM, { type: 'ready', runtime_generation: 'second', revision: '0' }); await flush()
      } else controller.dispose()
      pending.resolve({ activities: all.slice(0, 100).map(row => ({ ...row, text: 'Late repaired tool' })), has_more: false })
      await flush()
      expect(get(controller).snapshot?.activities.find(row => row.sequence === 50)?.text).toBe(outcome === 'newer-live' ? 'Newer live tool' : 'old')
      controller.dispose()
    }
  })

  it('repairs a scrollback page that was read while running but arrived after completion', async () => {
    const all = historySnapshot(Array.from({ length: 300 }, (_, index) => index + 1)).activities
    const pending = deferred<{ activities: ManagedSessionSnapshot['activities']; has_more: boolean }>()
    const initial = { ...snapshot('history'), runtime: { ...snapshot('history').runtime, activity: 'running' as const }, activities: all.slice(100, 200) }
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .resolve('getManagedSession', initial).handle('listManagedSessionActivity', () => pending.promise)
    const controller = createManagedSessionController(transport, 'history')
    controller.start(); await flush()
    transport.resolve('listManagedSessionActivity', { activities: all.slice(0, 200).map(row => ({ ...row, text: 'Completed tool' })), has_more: false })
    transport.resolve('getManagedSession', { ...snapshot('history'), activities: all.slice(200) })
    invalidate(transport, 'history'); await flush()
    pending.resolve({ activities: all.slice(0, 100), has_more: false })
    await flush(); await flush()
    expect(get(controller).snapshot?.activities.find(row => row.sequence === 1)?.text).toBe('Completed tool')
    controller.dispose()
  })

  it('warms scrollback once and retains historical row identity through live refreshes', async () => {
    const all = historySnapshot(Array.from({ length: 350 }, (_, index) => index + 1)).activities
      .map((row, index) => ({ ...row, kind: (index % 10 === 0 ? 'user_message' : 'agent_message') as 'user_message' | 'agent_message', turn_id: `turn-${Math.floor(index / 10)}` }))
    const initial = { ...snapshot('history'), activities: all.slice(-100) }
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .resolve('getManagedSession', initial)
      .resolve('listManagedSessionActivity', { activities: all.slice(50, 250), has_more: true })
    const controller = createManagedSessionController(transport, 'history')
    controller.start()
    await flush()
    expect(transport.callsFor('listManagedSessionActivity')).toHaveLength(1)
    const loaded = get(controller).snapshot!.activities
    expect(loaded).toHaveLength(300)
    const fresh = structuredClone(initial)
    fresh.activities[fresh.activities.length - 1].text = 'Latest live patch'
    transport.resolve('getManagedSession', fresh)
    for (let index = 0; index < 5; index += 1) { invalidate(transport, 'history'); await flush() }
    const current = get(controller).snapshot!.activities
    expect(transport.callsFor('listManagedSessionActivity')).toHaveLength(1)
    expect(current.filter((row, index) => row !== loaded[index])).toHaveLength(1)
    expect(current.at(-1)?.text).toBe('Latest live patch')
    controller.dispose()
  })

  it('retains older pages while newer live rows replace overlapping activity', async () => {
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true }).resolve('getManagedSession', historySnapshot([1001, 1002]))
    const controller = createManagedSessionController(transport, 'history')
    controller.start()
    await flush()
    transport.resolve('listManagedSessionActivity', { activities: historySnapshot([999, 1000]).activities, has_more: true })
    await controller.loadOlder()
    expect(transport.callsFor('listManagedSessionActivity')[0].input).toEqual({ session_id: 'history', before_sequence: 1001, limit: 1000, turn_limit: 20 })
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
    for (const action of [controller.startAgent, controller.cancel,
      () => controller.prompt('New work'), () => controller.respondPermission('permission', 'allow')]) {
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
      .resolve('cancelManagedPrompt', current)
      .resolve('respondManagedPermission', current)
    const controller = createManagedSessionController(transport, 'one')
    controller.start()
    await flush()
    await controller.startAgent()
    await controller.cancel()
    await controller.respondPermission('request-one', 'allow-once')
    await controller.respondPermission('request-two', null)
    expect(transport.callsFor('respondManagedPermission').map((call) => call.input)).toEqual([
      { session_id: 'one', request_id: 'request-one', option_id: 'allow-once' },
      { session_id: 'one', request_id: 'request-two', option_id: null },
    ])
    expect(transport.callsFor('startManagedSession')).toHaveLength(0)
    for (const name of ['cancelManagedPrompt'] as const) {
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


})

import { get } from 'svelte/store'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
import type { ManagedWorkspaceInfo } from '$lib/generated/feedback'
import { createManagedWorkspaceInfoController } from './managedWorkspaceInfoController'

async function flush() { for (let index = 0; index < 20; index += 1) await Promise.resolve() }
function deferred() {
  let resolve!: (value: ManagedWorkspaceInfo) => void
  const promise = new Promise<ManagedWorkspaceInfo>(done => { resolve = done })
  return { promise, resolve }
}

describe('visible Agent workspace metadata', () => {
  afterEach(() => vi.useRealTimers())

  it('refreshes the branch without responding to every streamed activity or changing the session scope', async () => {
    vi.useFakeTimers()
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .resolve('getManagedWorkspaceInfo', { cwd: '/project', branch: 'main' })
    const controller = createManagedWorkspaceInfoController(transport, 'one')
    const stop = controller.start()
    await flush()
    expect(get(controller)?.branch).toBe('main')
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'invalidate', runtime_generation: 'runtime', revision: '1', resources: [{ kind: 'managed_session', session_id: 'one' }] })
    await flush()
    expect(transport.calls).toHaveLength(1)
    transport.resolve('getManagedWorkspaceInfo', { cwd: '/project', branch: 'feature' })
    await vi.advanceTimersByTimeAsync(15_000)
    expect(get(controller)?.branch).toBe('feature')
    expect(transport.calls.every(call => call.name === 'getManagedWorkspaceInfo' && call.input?.session_id === 'one')).toBe(true)
    stop()
    await vi.advanceTimersByTimeAsync(30_000)
    expect(transport.calls).toHaveLength(2)
  })

  it('coalesces slow reads and ignores results after the view is disposed', async () => {
    vi.useFakeTimers()
    const pending = deferred()
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .handle('getManagedWorkspaceInfo', () => pending.promise)
    const controller = createManagedWorkspaceInfoController(transport, 'one')
    controller.start()
    await vi.advanceTimersByTimeAsync(45_000)
    expect(transport.calls).toHaveLength(1)
    controller.dispose()
    pending.resolve({ cwd: '/old', branch: 'stale' })
    await flush()
    expect(get(controller)).toBeNull()
  })

  it('follows a newly prepared draft session without leaking the previous project branch', async () => {
    vi.useFakeTimers()
    const previous = deferred()
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .handle('getManagedWorkspaceInfo', ({ session_id }) => session_id === 'old'
        ? previous.promise : { cwd: '/new', branch: 'feature' })
    const controller = createManagedWorkspaceInfoController(transport)
    controller.start()
    await flush()
    expect(transport.calls).toHaveLength(0)
    controller.setSessionId('old')
    await flush()
    controller.setSessionId('new')
    await flush()
    expect(get(controller)?.branch).toBe('feature')
    previous.resolve({ cwd: '/old', branch: 'main' })
    await flush()
    expect(get(controller)?.branch).toBe('feature')
    controller.setSessionId(null)
    await vi.advanceTimersByTimeAsync(15_000)
    expect(get(controller)).toBeNull()
    expect(transport.calls).toHaveLength(2)
    controller.dispose()
  })

  it('drops old-runtime results and clears a stale branch when the refreshed read fails', async () => {
    vi.useFakeTimers()
    const pending = deferred()
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
      .handle('getManagedWorkspaceInfo', () => pending.promise)
    const controller = createManagedWorkspaceInfoController(transport, 'one')
    controller.start()
    await flush()
    transport.resolve('getManagedWorkspaceInfo', { cwd: '/new', branch: 'current' })
    transport.emit(APPLICATION_EVENTS_STREAM, { type: 'ready', runtime_generation: 'new', revision: '0' })
    await flush()
    pending.resolve({ cwd: '/old', branch: 'stale' })
    await flush()
    expect(get(controller)).toEqual({ cwd: '/new', branch: 'current' })
    transport.handle('getManagedWorkspaceInfo', () => { throw new Error('Directory unavailable') })
    await vi.advanceTimersByTimeAsync(15_000)
    expect(get(controller)).toBeNull()
    controller.dispose()
  })
})

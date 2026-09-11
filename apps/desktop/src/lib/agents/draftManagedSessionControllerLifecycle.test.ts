import { get } from 'svelte/store'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import type { ManagedSessionSnapshot } from '$lib/generated/feedback'
import { createDraftManagedSessionController } from './draftManagedSessionController'
import {
  deferred,
  flush,
  releaseDiagnostics,
  setup,
  snapshot,
} from './draftManagedSessionControllerTestHarness'

afterEach(() => {
  releaseDiagnostics()
})

describe('managed draft lifecycle: send, retry, close, promote', () => {
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
})

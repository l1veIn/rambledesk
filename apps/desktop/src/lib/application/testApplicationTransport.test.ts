import { describe, expect, expectTypeOf, it, vi } from 'vitest'
import type {
  HostSessionSummary,
  RenameHostSessionInput,
} from '../generated/feedback'
import { defineApplicationStream } from './applicationTransport'
import { TestApplicationTransport } from './testApplicationTransport'

const hostSession: HostSessionSummary = {
  host_id: 'codex',
  host_session_id: 'session-1',
  title: 'First session',
  source_hint: null,
  request_count: 1,
  updated_at: '2026-09-01T01:02:03Z',
  pinned_at: null,
  host_pinned_at: null,
  pending_count: 0,
  archived_at: null,
}

describe('TestApplicationTransport', () => {
  it('dispatches typed calls and records their semantic inputs', async () => {
    const transport = new TestApplicationTransport({ mode: 'test' })
    transport.handle('renameHostSession', (input) => {
      expectTypeOf(input).toEqualTypeOf<RenameHostSessionInput>()
      return { ...hostSession, title: input.title }
    })

    const result = await transport.call('renameHostSession', {
      host_id: 'codex',
      host_session_id: 'session-1',
      title: 'Renamed session',
    })

    expect(result.title).toBe('Renamed session')
    expect(transport.callsFor('renameHostSession')).toEqual([
      {
        name: 'renameHostSession',
        input: {
          host_id: 'codex',
          host_session_id: 'session-1',
          title: 'Renamed session',
        },
      },
    ])
  })

  it('supports resolved handlers and rejected handlers per command', async () => {
    const reason = new Error('save failed')
    const transport = new TestApplicationTransport(undefined)
      .resolve('listHostSessions', [hostSession])
      .reject('saveFeedbackDraft', reason)

    await expect(transport.call('listHostSessions', undefined)).resolves.toEqual([hostSession])
    await expect(
      transport.call('saveFeedbackDraft', {
        request_id: 'request-1',
        expected_revision: 1,
        document_json: '{}',
        body_markdown: '',
      }),
    ).rejects.toBe(reason)
  })

  it('holds readiness until explicitly released', async () => {
    const transport = new TestApplicationTransport(undefined)
    let ready = false
    const waiting = transport.waitUntilReady().then(() => {
      ready = true
    })

    await Promise.resolve()
    expect(ready).toBe(false)

    transport.markReady()
    await waiting
    expect(ready).toBe(true)

    transport.markReady()
    await expect(transport.waitUntilReady()).resolves.toBeUndefined()
  })

  it('emits typed events and stops after unsubscribe', () => {
    type RequestChanged = Readonly<{ requestId: string }>
    const stream = defineApplicationStream<RequestChanged>('test:request-changed')
    const sameIdDifferentStream = defineApplicationStream<RequestChanged>('test:request-changed')
    const handler = vi.fn<(event: RequestChanged) => void>()
    const transport = new TestApplicationTransport(undefined)
    const unsubscribe = transport.subscribe(stream, handler)

    transport.emit(sameIdDifferentStream, { requestId: 'ignored' })
    transport.emit(stream, { requestId: 'request-1' })
    unsubscribe()
    unsubscribe()
    transport.emit(stream, { requestId: 'request-2' })

    expect(handler).toHaveBeenCalledTimes(1)
    expect(handler).toHaveBeenCalledWith({ requestId: 'request-1' })
  })

  it('returns the injected capability manifest unchanged', () => {
    const capabilities = { testCapability: true } as const
    const transport = new TestApplicationTransport(capabilities, { initiallyReady: true })

    expect(transport.capabilities()).toBe(capabilities)
    expectTypeOf(transport.capabilities()).toEqualTypeOf<typeof capabilities>()
  })
})

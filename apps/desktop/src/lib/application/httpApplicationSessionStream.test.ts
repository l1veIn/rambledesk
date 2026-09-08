import { describe, expect, it, vi } from 'vitest'

import {
  APPLICATION_EVENT_PROTOCOL,
  APPLICATION_EVENTS_STREAM,
  REVISION_HEADER,
  RUNTIME_GENERATION_HEADER,
} from './applicationEvents'
import {
  HttpApplicationSessionRevokedError,
  HttpApplicationSession,
  HttpApplicationTransport,
  StaleHttpApplicationLeaseError,
  StaleHttpApplicationResponseError,
  type ApplicationWebSocket,
} from './httpApplicationTransport'
import { APPLICATION_CONFORMANCE_INPUTS } from './applicationTransportConformance'
import {
  ControlledWebSocket,
  expectOlderSemanticProjectionRejected,
  flush,
  response,
} from './httpApplicationSessionTestHarness'

describe('HttpApplicationSession stream, auth and runtime generation', () => {
  it.each([
    { resources: [{ kind: 'navigation' }] },
    { resources: [{ kind: 'feedback_workspace', request_id: 'request-1' }] },
  ] as const)(
    'returns a committed mutation once after later invalidation resources %j',
    async ({ resources }) => {
      const socket = new ControlledWebSocket()
      let socketCreated = false
      let bodyStarted: (() => void) | undefined
      const started = new Promise<void>((resolve) => {
        bodyStarted = resolve
      })
      let resolveBody: ((value: unknown) => void) | undefined
      const body = new Promise<unknown>((resolve) => {
        resolveBody = resolve
      })
      const delayed = response({}, 'runtime-a', '6')
      vi.spyOn(delayed, 'json').mockImplementation(async () => {
        bodyStarted?.()
        return body
      })
      const fetchImplementation = vi.fn<typeof fetch>(async (url) =>
        String(url).endsWith('/api/health')
          ? Response.json({ runtime_generation: 'runtime-a', revision: '5' })
          : delayed,
      )
      const session = HttpApplicationSession.authenticated({
        accessToken: 'session-token',
        pageUrl: 'https://workbench.example/app',
        fetch: fetchImplementation,
        webSocket: () => {
          socketCreated = true
          return socket
        },
      })
      const transport = new HttpApplicationTransport(session.lease())
      await vi.waitFor(() => expect(socketCreated).toBe(true))
      socket.emit({ type: 'ready', runtime_generation: 'runtime-a', revision: '5' })
      const pending = transport.call('saveFeedbackDraft', {
        request_id: 'request-1',
        document_json: '{}',
        body_markdown: 'saved',
        expected_revision: 0,
      })
      await started
      socket.emit({
        type: 'invalidate',
        runtime_generation: 'runtime-a',
        revision: '7',
        resources: [...resources],
      })
      const saved = { saved_revision: 1, updated_at: null }
      resolveBody?.(saved)

      await expect(pending).resolves.toEqual(saved)
      expect(fetchImplementation).toHaveBeenCalledTimes(2)
    },
  )

  it.each([
    { type: 'ready', revision: '0' },
    { runtime_generation: 'runtime-a', revision: '0' },
    { type: 'ready', runtime_generation: 'runtime-a', revision: 0 },
    { type: 'ready', runtime_generation: 'runtime-a', revision: 'not-decimal' },
    {
      type: 'invalidate',
      runtime_generation: 'runtime-a',
      revision: '1',
      resources: 'navigation',
    },
    {
      type: 'invalidate',
      runtime_generation: 'runtime-a',
      revision: '1',
      resources: [{ kind: 'feedback_workspace' }],
    },
  ])('reports and disconnects a malformed application event: %j', async (event) => {
    const socket = new ControlledWebSocket()
    let socketCreated = false
    const onError = vi.fn()
    const session = HttpApplicationSession.authenticated({
      accessToken: 'session-token',
      pageUrl: 'https://workbench.example/app',
      fetch: vi.fn<typeof fetch>(async () =>
        Response.json({ runtime_generation: 'runtime-a', revision: '0' }),
      ),
      webSocket: () => {
        socketCreated = true
        return socket
      },
      scheduleReconnect: () => () => undefined,
    })
    const transport = new HttpApplicationTransport(session.lease())
    transport.subscribe(APPLICATION_EVENTS_STREAM, vi.fn(), onError)
    const waiting = transport.waitUntilReady()
    await vi.waitFor(() => expect(socketCreated).toBe(true))
    socket.emit(event)

    await expect(waiting).rejects.toBeInstanceOf(StaleHttpApplicationLeaseError)
    expect(onError).toHaveBeenCalledWith(expect.any(Error))
    expect(socket.closed).toBe(true)
  })

  it('reports and disconnects a non-text application event frame', async () => {
    const socket = new ControlledWebSocket()
    let socketCreated = false
    const onError = vi.fn()
    const session = HttpApplicationSession.authenticated({
      accessToken: 'session-token',
      pageUrl: 'https://workbench.example/app',
      fetch: vi.fn<typeof fetch>(async () =>
        Response.json({ runtime_generation: 'runtime-a', revision: '0' }),
      ),
      webSocket: () => {
        socketCreated = true
        return socket
      },
      scheduleReconnect: () => () => undefined,
    })
    const transport = new HttpApplicationTransport(session.lease())
    transport.subscribe(APPLICATION_EVENTS_STREAM, vi.fn(), onError)
    const waiting = transport.waitUntilReady()
    await vi.waitFor(() => expect(socketCreated).toBe(true))
    socket.emitRaw(new Uint8Array([1, 2, 3]))

    await expect(waiting).rejects.toBeInstanceOf(StaleHttpApplicationLeaseError)
    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ message: expect.stringContaining('non-text') }))
    expect(socket.closed).toBe(true)
  })

  it('disconnects when an application event subscriber throws', async () => {
    const socket = new ControlledWebSocket()
    let socketCreated = false
    const onError = vi.fn()
    const session = HttpApplicationSession.authenticated({
      accessToken: 'session-token',
      pageUrl: 'https://workbench.example/app',
      fetch: vi.fn<typeof fetch>(async () =>
        Response.json({ runtime_generation: 'runtime-a', revision: '0' }),
      ),
      webSocket: () => {
        socketCreated = true
        return socket
      },
      scheduleReconnect: () => () => undefined,
    })
    const transport = new HttpApplicationTransport(session.lease())
    transport.subscribe(
      APPLICATION_EVENTS_STREAM,
      (event) => {
        if (event.type === 'invalidate') throw new Error('subscriber failed')
      },
      onError,
    )
    await vi.waitFor(() => expect(socketCreated).toBe(true))
    socket.emit({ type: 'ready', runtime_generation: 'runtime-a', revision: '0' })
    await transport.waitUntilReady()
    socket.emit({
      type: 'invalidate',
      runtime_generation: 'runtime-a',
      revision: '1',
      resources: [{ kind: 'navigation' }],
    })

    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ message: 'subscriber failed' }))
    expect(socket.closed).toBe(true)
  })

  it('drops low revisions and refetches all resources after same-generation reconnect', async () => {
    const sockets: ControlledWebSocket[] = []
    const reconnects: Array<() => void> = []
    const fetchImplementation = vi.fn<typeof fetch>().mockImplementation(async () =>
      Response.json({ runtime_generation: 'runtime-a', revision: '5' }),
    )
    const session = HttpApplicationSession.authenticated({
      accessToken: 'session-token',
      pageUrl: 'https://workbench.example/app',
      fetch: fetchImplementation,
      webSocket: () => {
        const socket = new ControlledWebSocket()
        sockets.push(socket)
        return socket
      },
      scheduleReconnect: (callback) => {
        reconnects.push(callback)
        return () => undefined
      },
      random: () => 0.5,
    })
    const transport = new HttpApplicationTransport(session.lease())
    const events: unknown[] = []
    transport.subscribe(APPLICATION_EVENTS_STREAM, (event) => events.push(event), vi.fn())
    await vi.waitFor(() => expect(sockets).toHaveLength(1))
    sockets[0]!.emit({ type: 'ready', runtime_generation: 'runtime-a', revision: '5' })
    sockets[0]!.emit({
      type: 'invalidate',
      runtime_generation: 'runtime-a',
      revision: '4',
      resources: [{ kind: 'navigation' }],
    })
    expect(events).toHaveLength(1)

    sockets[0]!.disconnect()
    expect(reconnects).toHaveLength(1)
    reconnects[0]!()
    await vi.waitFor(() => expect(sockets).toHaveLength(2))
    sockets[1]!.emit({ type: 'ready', runtime_generation: 'runtime-a', revision: '5' })
    expect(events).toContainEqual({
      type: 'invalidate',
      runtime_generation: 'runtime-a',
      revision: '5',
      resources: [{ kind: 'all' }],
    })
  })

  it('disconnects on a response from a new runtime generation and refetches after ready', async () => {
    const sockets: ControlledWebSocket[] = []
    const reconnects: Array<() => void> = []
    let healthAttempts = 0
    let applicationCalls = 0
    const fetchImplementation = vi.fn<typeof fetch>(async (url) => {
      if (String(url).endsWith('/api/health')) {
        healthAttempts += 1
        return Response.json({
          runtime_generation: healthAttempts === 1 ? 'runtime-a' : 'runtime-b',
          revision: healthAttempts === 1 ? '5' : '0',
        })
      }
      applicationCalls += 1
      return response({ saved_revision: 1, updated_at: null }, 'runtime-b', '1')
    })
    const session = HttpApplicationSession.authenticated({
      accessToken: 'session-token',
      pageUrl: 'https://workbench.example/app',
      fetch: fetchImplementation,
      webSocket: () => {
        const socket = new ControlledWebSocket()
        sockets.push(socket)
        return socket
      },
      scheduleReconnect: (callback) => {
        reconnects.push(callback)
        return () => undefined
      },
    })
    const transport = new HttpApplicationTransport(session.lease())
    const events: unknown[] = []
    transport.subscribe(APPLICATION_EVENTS_STREAM, (event) => events.push(event), vi.fn())
    await vi.waitFor(() => expect(sockets).toHaveLength(1))
    sockets[0]!.emit({ type: 'ready', runtime_generation: 'runtime-a', revision: '5' })

    await expect(
      transport.call('saveFeedbackDraft', {
        request_id: '0195f7e2-5c31-7b5a-8ab7-3c84ea4fc827',
        document_json: '{}',
        body_markdown: 'saved by the replacement runtime',
        expected_revision: 0,
      }),
    ).rejects.toBeInstanceOf(StaleHttpApplicationLeaseError)
    expect(applicationCalls).toBe(1)
    expect(sockets[0]!.closed).toBe(true)
    expect(reconnects).toHaveLength(1)

    reconnects[0]!()
    await vi.waitFor(() => expect(sockets).toHaveLength(2))
    sockets[1]!.emit({ type: 'ready', runtime_generation: 'runtime-b', revision: '1' })
    expect(events).toContainEqual({
      type: 'invalidate',
      runtime_generation: 'runtime-b',
      revision: '1',
      resources: [{ kind: 'all' }],
    })
    expect(applicationCalls).toBe(1)
  })

  it('aborts old-epoch HTTP and resets the ledger for a new runtime generation', async () => {
    const sockets: ControlledWebSocket[] = []
    const reconnects: Array<() => void> = []
    let applicationSignal: AbortSignal | undefined
    const fetchImplementation = vi.fn<typeof fetch>((url, init) => {
      if (String(url).endsWith('/api/health')) {
        return Promise.resolve(Response.json({ runtime_generation: 'runtime-a', revision: '8' }))
      }
      applicationSignal = init?.signal as AbortSignal
      return new Promise<Response>((_resolve, reject) => {
        applicationSignal?.addEventListener('abort', () => reject(applicationSignal?.reason))
      })
    })
    const session = HttpApplicationSession.authenticated({
      accessToken: 'session-token',
      pageUrl: 'https://workbench.example/app',
      fetch: fetchImplementation,
      webSocket: () => {
        const socket = new ControlledWebSocket()
        sockets.push(socket)
        return socket
      },
      scheduleReconnect: (callback) => {
        reconnects.push(callback)
        return () => undefined
      },
    })
    const transport = new HttpApplicationTransport(session.lease())
    const events: unknown[] = []
    transport.subscribe(APPLICATION_EVENTS_STREAM, (event) => events.push(event), vi.fn())
    await vi.waitFor(() => expect(sockets).toHaveLength(1))
    sockets[0]!.emit({ type: 'ready', runtime_generation: 'runtime-a', revision: '8' })
    const pending = transport.call('listFeedbackInbox', undefined)
    await flush()
    sockets[0]!.disconnect()
    expect(applicationSignal?.aborted).toBe(true)
    await expect(pending).rejects.toBeInstanceOf(StaleHttpApplicationLeaseError)

    reconnects[0]!()
    await vi.waitFor(() => expect(sockets).toHaveLength(2))
    sockets[1]!.emit({ type: 'ready', runtime_generation: 'runtime-b', revision: '1' })
    expect(events).toContainEqual({
      type: 'invalidate',
      runtime_generation: 'runtime-b',
      revision: '1',
      resources: [{ kind: 'all' }],
    })
  })

  it('does not replay a stale-generation mutation', async () => {
    const socket = new ControlledWebSocket()
    let socketCreated = false
    const fetchImplementation = vi.fn<typeof fetch>(async (url) => {
      if (String(url).endsWith('/api/health')) {
        return Response.json({ runtime_generation: 'runtime-a', revision: '0' })
      }
      return response(
        {
          code: 'RUNTIME_GENERATION_STALE',
          message: 'refetch',
          retryable: false,
        },
        'runtime-a',
        '0',
        409,
      )
    })
    const session = HttpApplicationSession.authenticated({
      accessToken: 'session-token',
      pageUrl: 'https://workbench.example/app',
      fetch: fetchImplementation,
      webSocket: () => {
        socketCreated = true
        return socket
      },
    })
    const transport = new HttpApplicationTransport(session.lease())
    await vi.waitFor(() => expect(socketCreated).toBe(true))
    socket.emit({ type: 'ready', runtime_generation: 'runtime-a', revision: '0' })
    await expect(
      transport.call('saveFeedbackDraft', {
        request_id: 'request-1',
        document_json: '{}',
        body_markdown: 'draft',
        expected_revision: 0,
      }),
    ).rejects.toMatchObject({ code: 'RUNTIME_GENERATION_STALE' })
    expect(fetchImplementation).toHaveBeenCalledTimes(2)
  })
})

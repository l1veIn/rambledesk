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

class ControlledWebSocket implements ApplicationWebSocket {
  protocol = APPLICATION_EVENT_PROTOCOL
  readyState: WebSocket['readyState'] = 1
  closed = false
  readonly #listeners = new Map<string, Set<EventListener>>()

  addEventListener(type: string, listener: EventListenerOrEventListenerObject): void {
    let listeners = this.#listeners.get(type)
    if (!listeners) {
      listeners = new Set()
      this.#listeners.set(type, listeners)
    }
    listeners.add(listener as EventListener)
  }

  removeEventListener(type: string, listener: EventListenerOrEventListenerObject): void {
    this.#listeners.get(type)?.delete(listener as EventListener)
  }

  close(): void {
    this.closed = true
  }

  emit(event: unknown): void {
    this.#dispatch('message', new MessageEvent('message', { data: JSON.stringify(event) }))
  }

  emitRaw(data: unknown): void {
    this.#dispatch('message', new MessageEvent('message', { data }))
  }

  disconnect(): void {
    this.#dispatch('close', new Event('close'))
  }

  #dispatch(type: string, event: Event): void {
    for (const listener of [...(this.#listeners.get(type) ?? [])]) listener(event)
  }
}

function response(
  value: unknown,
  generation: string,
  revision: string,
  status = 200,
): Response {
  return Response.json(value, {
    status,
    headers: {
      [RUNTIME_GENERATION_HEADER]: generation,
      [REVISION_HEADER]: revision,
    },
  })
}

async function flush(): Promise<void> {
  await Promise.resolve()
  await Promise.resolve()
}

async function expectOlderSemanticProjectionRejected(
  first: (transport: HttpApplicationTransport) => Promise<unknown>,
  second: (transport: HttpApplicationTransport) => Promise<unknown>,
): Promise<void> {
  const socket = new ControlledWebSocket()
  let socketCreated = false
  const responses = [response({}, 'runtime-a', '7'), response({}, 'runtime-a', '6')]
  let applicationCalls = 0
  const session = HttpApplicationSession.authenticated({
    accessToken: 'session-token',
    pageUrl: 'https://workbench.example/app',
    fetch: vi.fn<typeof fetch>(async (url) => {
      if (String(url).endsWith('/api/health')) {
        return Response.json({ runtime_generation: 'runtime-a', revision: '5' })
      }
      const next = responses[applicationCalls]
      applicationCalls += 1
      return next!
    }),
    webSocket: () => {
      socketCreated = true
      return socket
    },
  })
  const transport = new HttpApplicationTransport(session.lease())
  await vi.waitFor(() => expect(socketCreated).toBe(true))
  socket.emit({ type: 'ready', runtime_generation: 'runtime-a', revision: '5' })

  await expect(first(transport)).resolves.toEqual({})
  await expect(second(transport)).rejects.toBeInstanceOf(StaleHttpApplicationResponseError)
  expect(applicationCalls).toBe(2)
}

describe('HttpApplicationSession reconnect state machine', () => {
  it('reports a prior terminal revocation to late subscribers without retaining them', async () => {
    const onTerminalError = vi.fn()
    const session = HttpApplicationSession.authenticated({
      accessToken: 'session-token',
      pageUrl: 'https://workbench.example/app',
      fetch: vi.fn<typeof fetch>(async () => new Response(null, { status: 401 })),
      webSocket: () => new ControlledWebSocket(),
      scheduleReconnect: () => () => undefined,
      onTerminalError,
    })
    const transport = new HttpApplicationTransport(session.lease())
    let revoked: unknown
    try {
      await transport.waitUntilReady()
    } catch (cause) {
      revoked = cause
    }
    expect(revoked).toBeInstanceOf(HttpApplicationSessionRevokedError)
    expect(onTerminalError).toHaveBeenCalledTimes(1)
    expect(onTerminalError).toHaveBeenCalledWith(revoked)

    const onError = vi.fn()
    const handler = vi.fn()
    transport.subscribe(APPLICATION_EVENTS_STREAM, handler, onError)
    expect(onError).not.toHaveBeenCalled()
    await flush()
    expect(onError).toHaveBeenCalledTimes(1)
    expect(onError).toHaveBeenCalledWith(revoked)
    expect(handler).not.toHaveBeenCalled()

    const cancelledError = vi.fn()
    const unsubscribe = transport.subscribe(APPLICATION_EVENTS_STREAM, handler, cancelledError)
    unsubscribe()
    await flush()
    expect(cancelledError).not.toHaveBeenCalled()
  })

  it('recovers from an initial health failure and requests a full snapshot', async () => {
    const sockets: ControlledWebSocket[] = []
    const reconnects: Array<() => void> = []
    let healthAttempts = 0
    const session = HttpApplicationSession.authenticated({
      accessToken: 'session-token',
      pageUrl: 'https://workbench.example/app',
      fetch: vi.fn<typeof fetch>(async () => {
        healthAttempts += 1
        if (healthAttempts === 1) throw new Error('temporary health failure')
        return Response.json({ runtime_generation: 'runtime-a', revision: '0' })
      }),
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
    const onError = vi.fn()
    transport.subscribe(APPLICATION_EVENTS_STREAM, (event) => events.push(event), onError)
    await vi.waitFor(() => expect(reconnects).toHaveLength(1))
    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ message: 'temporary health failure' }))

    reconnects[0]!()
    await vi.waitFor(() => expect(sockets).toHaveLength(1))
    sockets[0]!.emit({ type: 'ready', runtime_generation: 'runtime-a', revision: '0' })

    expect(events).toContainEqual({
      type: 'invalidate',
      runtime_generation: 'runtime-a',
      revision: '0',
      resources: [{ kind: 'all' }],
    })
  })

  it('blocks commands until authenticated ready and sends mutations with the generation', async () => {
    const sockets: ControlledWebSocket[] = []
    const fetchImplementation = vi.fn<typeof fetch>(async (url) => {
      if (String(url).endsWith('/api/health')) {
        return Response.json({ runtime_generation: 'runtime-a', revision: '0' })
      }
      return response({ saved_revision: 1, updated_at: null }, 'runtime-a', '1')
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
    })
    const transport = new HttpApplicationTransport(session.lease())
    const pending = transport.call('saveFeedbackDraft', {
      request_id: 'request-1',
      document_json: '{}',
      body_markdown: 'draft',
      expected_revision: 0,
    })
    await flush()
    expect(fetchImplementation).toHaveBeenCalledTimes(1)
    expect(fetchImplementation.mock.calls[0]![1]?.method).toBe('POST')
    expect(new Headers(fetchImplementation.mock.calls[0]![1]?.headers).has('Origin')).toBe(false)
    await vi.waitFor(() => expect(sockets).toHaveLength(1))

    sockets[0]!.emit({
      type: 'ready',
      runtime_generation: 'runtime-a',
      revision: '0',
    })
    await pending
    expect(fetchImplementation).toHaveBeenCalledTimes(2)
    expect(new Headers(fetchImplementation.mock.calls[1]![1]?.headers).get(RUNTIME_GENERATION_HEADER))
      .toBe('runtime-a')
  })

  it.each([401, 403] as const)(
    'terminally revokes a ready session after HTTP %i',
    async (status) => {
      const socket = new ControlledWebSocket()
      const reconnects: Array<() => void> = []
      let socketCreated = false
      let applicationCalls = 0
      let releaseRejection: (() => void) | undefined
      const rejectionReady = new Promise<void>((resolve) => {
        releaseRejection = resolve
      })
      const fetchImplementation = vi.fn<typeof fetch>(async (url, init) => {
        if (String(url).endsWith('/api/health')) {
          return Response.json({ runtime_generation: 'runtime-a', revision: '5' })
        }
        applicationCalls += 1
        if (applicationCalls === 1) {
          await rejectionReady
          return new Response(null, { status })
        }
        return new Promise<Response>((_resolve, reject) => {
          init?.signal?.addEventListener('abort', () => reject(init.signal?.reason))
        })
      })
      const session = HttpApplicationSession.authenticated({
        accessToken: 'session-token',
        pageUrl: 'https://workbench.example/app',
        fetch: fetchImplementation,
        webSocket: () => {
          socketCreated = true
          return socket
        },
        scheduleReconnect: (callback) => {
          reconnects.push(callback)
          return () => undefined
        },
      })
      const transport = new HttpApplicationTransport(session.lease())
      const onError = vi.fn()
      transport.subscribe(APPLICATION_EVENTS_STREAM, vi.fn(), onError)
      await vi.waitFor(() => expect(socketCreated).toBe(true))
      socket.emit({ type: 'ready', runtime_generation: 'runtime-a', revision: '5' })

      const mutation = transport.call('saveFeedbackDraft', {
        request_id: '0195f7e2-5c31-7b5a-8ab7-3c84ea4fc827',
        document_json: '{}',
        body_markdown: 'must not replay',
        expected_revision: 0,
      })
      const concurrentQuery = transport.call('listHostSessions', undefined)
      const mutationRejected = expect(mutation).rejects.toMatchObject({ status })
      const queryRejected = expect(concurrentQuery).rejects.toBeInstanceOf(
        HttpApplicationSessionRevokedError,
      )
      await vi.waitFor(() => expect(applicationCalls).toBe(2))
      releaseRejection?.()

      await mutationRejected
      await queryRejected
      await expect(transport.waitUntilReady()).rejects.toBeInstanceOf(
        HttpApplicationSessionRevokedError,
      )
      await expect(transport.call('listFeedbackInbox', undefined)).rejects.toBeInstanceOf(
        HttpApplicationSessionRevokedError,
      )
      expect(applicationCalls).toBe(2)
      expect(socket.closed).toBe(true)
      expect(reconnects).toHaveLength(0)
      expect(onError).toHaveBeenCalledWith(expect.any(HttpApplicationSessionRevokedError))
    },
  )

  it('still delivers the event matching a mutation response revision', async () => {
    const socket = new ControlledWebSocket()
    let socketCreated = false
    const fetchImplementation = vi.fn<typeof fetch>(async (url) => {
      if (String(url).endsWith('/api/health')) {
        return Response.json({ runtime_generation: 'runtime-a', revision: '0' })
      }
      return response({ saved_revision: 1, updated_at: null }, 'runtime-a', '1')
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
    const events: unknown[] = []
    transport.subscribe(APPLICATION_EVENTS_STREAM, (event) => events.push(event), vi.fn())
    await vi.waitFor(() => expect(socketCreated).toBe(true))
    socket.emit({ type: 'ready', runtime_generation: 'runtime-a', revision: '0' })
    await transport.call('saveFeedbackDraft', {
      request_id: 'request-1',
      document_json: '{}',
      body_markdown: 'draft',
      expected_revision: 0,
    })
    socket.emit({
      type: 'invalidate',
      runtime_generation: 'runtime-a',
      revision: '1',
      resources: [{ kind: 'feedback_workspace', request_id: 'request-1' }],
    })

    expect(events).toContainEqual({
      type: 'invalidate',
      runtime_generation: 'runtime-a',
      revision: '1',
      resources: [{ kind: 'feedback_workspace', request_id: 'request-1' }],
    })
  })

  it('accepts delayed navigation JSON after an unrelated workspace invalidation', async () => {
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
    const delayed = response([], 'runtime-a', '5')
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
    const pending = transport.call('listHostSessions', undefined)
    await started
    socket.emit({
      type: 'invalidate',
      runtime_generation: 'runtime-a',
      revision: '6',
      resources: [{ kind: 'feedback_workspace', request_id: 'request-1' }],
    })
    resolveBody?.([])

    await expect(pending).resolves.toEqual([])
  })

  it('rejects a delayed request projection after its host session is deleted', async () => {
    const canonicalRequestId = '0195f7e2-5c31-7b5a-8ab7-3c84ea4fc827'
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
    const delayed = response({}, 'runtime-a', '5')
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
    const pending = transport.call('getFeedbackWorkspace', {
      request_id: canonicalRequestId.replaceAll('-', '').toUpperCase(),
    })
    await started
    socket.emit({
      type: 'invalidate',
      runtime_generation: 'runtime-a',
      revision: '6',
      resources: [
        { kind: 'navigation' },
        {
          kind: 'host_session_resources',
          host_id: 'codex',
          host_session_id: 'session-1',
        },
        { kind: 'feedback_workspace', request_id: canonicalRequestId },
        { kind: 'published_feedback', request_id: canonicalRequestId },
      ],
    })
    resolveBody?.({})

    await expect(pending).rejects.toBeInstanceOf(StaleHttpApplicationResponseError)
    expect(fetchImplementation).toHaveBeenCalledTimes(2)
  })

  it('rejects delayed binary bytes older than an invalidation applied while decoding', async () => {
    const socket = new ControlledWebSocket()
    let socketCreated = false
    let bodyStarted: (() => void) | undefined
    const started = new Promise<void>((resolve) => {
      bodyStarted = resolve
    })
    let resolveBody: ((value: ArrayBuffer) => void) | undefined
    const body = new Promise<ArrayBuffer>((resolve) => {
      resolveBody = resolve
    })
    const delayed = new Response(new Uint8Array([1]), {
      headers: {
        [RUNTIME_GENERATION_HEADER]: 'runtime-a',
        [REVISION_HEADER]: '5',
      },
    })
    vi.spyOn(delayed, 'arrayBuffer').mockImplementation(async () => {
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
    const pending = transport.call('readFeedbackAttachment', {
      request_id: 'request-1',
      attachment_id: 'attachment-1',
    })
    await started
    socket.emit({
      type: 'invalidate',
      runtime_generation: 'runtime-a',
      revision: '6',
      resources: [{ kind: 'feedback_workspace', request_id: 'request-1' }],
    })
    resolveBody?.(new Uint8Array([1]).buffer)

    await expect(pending).rejects.toBeInstanceOf(StaleHttpApplicationResponseError)
    expect(fetchImplementation).toHaveBeenCalledTimes(2)
  })

  it('rejects a lower HTTP projection that completes after a newer projection', async () => {
    const socket = new ControlledWebSocket()
    let socketCreated = false
    const starts: Array<() => void> = []
    const releases: Array<(value: unknown) => void> = []
    const bodies = [0, 1].map(
      (index) => new Promise<unknown>((resolve) => {
        releases[index] = resolve
      }),
    )
    const responses = [response([], 'runtime-a', '7'), response([], 'runtime-a', '6')]
    responses.forEach((entry, index) => {
      vi.spyOn(entry, 'json').mockImplementation(async () => {
        starts[index]?.()
        return bodies[index]
      })
    })
    let applicationCalls = 0
    const session = HttpApplicationSession.authenticated({
      accessToken: 'session-token',
      pageUrl: 'https://workbench.example/app',
      fetch: vi.fn<typeof fetch>(async (url) => {
        if (String(url).endsWith('/api/health')) {
          return Response.json({ runtime_generation: 'runtime-a', revision: '5' })
        }
        const response = responses[applicationCalls]
        applicationCalls += 1
        return response!
      }),
      webSocket: () => {
        socketCreated = true
        return socket
      },
    })
    const transport = new HttpApplicationTransport(session.lease())
    await vi.waitFor(() => expect(socketCreated).toBe(true))
    socket.emit({ type: 'ready', runtime_generation: 'runtime-a', revision: '5' })
    const bodyStarted = [0, 1].map(
      (index) => new Promise<void>((resolve) => {
        starts[index] = resolve
      }),
    )
    const newer = transport.call('listHostSessions', undefined)
    const older = transport.call('listHostSessions', undefined)
    await Promise.all(bodyStarted)
    releases[0]?.([])
    await expect(newer).resolves.toEqual([])
    releases[1]?.([])

    await expect(older).rejects.toBeInstanceOf(StaleHttpApplicationResponseError)
    expect(applicationCalls).toBe(2)
  })

  it.each([
    {
      label: 'list defaults and explicit defaults',
      first: (transport: HttpApplicationTransport) =>
        transport.call('listFeedbackRequests', {
          host_id: null,
          host_session_id: null,
          status: null,
          archived: null,
          search: null,
          limit: null,
          cursor: null,
        }),
      second: (transport: HttpApplicationTransport) =>
        transport.call('listFeedbackRequests', {
          host_id: null,
          host_session_id: null,
          status: ['waiting', 'in_progress'],
          archived: false,
          search: null,
          limit: 50,
          cursor: null,
        }),
    },
    {
      label: 'empty and absent search',
      first: (transport: HttpApplicationTransport) =>
        transport.call('listArchivedHostSessions', { search: null }),
      second: (transport: HttpApplicationTransport) =>
        transport.call('listArchivedHostSessions', { search: '   ' }),
    },
    {
      label: 'trimmed search',
      first: (transport: HttpApplicationTransport) =>
        transport.call('listArchivedHostSessions', { search: 'needle' }),
      second: (transport: HttpApplicationTransport) =>
        transport.call('listArchivedHostSessions', { search: '  needle  ' }),
    },
    {
      label: 'duplicated and reordered statuses',
      first: (transport: HttpApplicationTransport) =>
        transport.call('listFeedbackRequests', {
          host_id: 'codex',
          host_session_id: 'session-1',
          status: ['waiting', 'completed'],
          archived: false,
          search: null,
          limit: 50,
          cursor: null,
        }),
      second: (transport: HttpApplicationTransport) =>
        transport.call('listFeedbackRequests', {
          host_id: 'codex',
          host_session_id: 'session-1',
          status: ['completed', 'waiting', 'waiting'],
          archived: false,
          search: null,
          limit: 50,
          cursor: null,
        }),
    },
    {
      label: 'UUID alternate spelling',
      first: (transport: HttpApplicationTransport) =>
        transport.call('getFeedbackWorkspace', {
          request_id: '0195f7e2-5c31-7b5a-8ab7-3c84ea4fc827',
        }),
      second: (transport: HttpApplicationTransport) =>
        transport.call('getFeedbackWorkspace', {
          request_id: '0195F7E25C317B5A8AB73C84EA4FC827',
        }),
    },
  ])('rejects r6 after r7 for semantic projection aliases: $label', async ({ first, second }) => {
    await expectOlderSemanticProjectionRejected(first, second)
  })

  it('keeps completed HTTP watermarks independent across navigation operations', async () => {
    const socket = new ControlledWebSocket()
    let socketCreated = false
    const responses = [
      response({ requests: [], next_cursor: null }, 'runtime-a', '7'),
      response([], 'runtime-a', '6'),
    ]
    let applicationCalls = 0
    const session = HttpApplicationSession.authenticated({
      accessToken: 'session-token',
      pageUrl: 'https://workbench.example/app',
      fetch: vi.fn<typeof fetch>(async (url) => {
        if (String(url).endsWith('/api/health')) {
          return Response.json({ runtime_generation: 'runtime-a', revision: '5' })
        }
        const next = responses[applicationCalls]
        applicationCalls += 1
        return next!
      }),
      webSocket: () => {
        socketCreated = true
        return socket
      },
    })
    const transport = new HttpApplicationTransport(session.lease())
    await vi.waitFor(() => expect(socketCreated).toBe(true))
    socket.emit({ type: 'ready', runtime_generation: 'runtime-a', revision: '5' })

    await expect(
      transport.call('listFeedbackRequests', {
        host_id: null,
        host_session_id: null,
        status: null,
        archived: null,
        search: null,
        limit: null,
        cursor: null,
      }),
    ).resolves.toEqual({ requests: [], next_cursor: null })
    await expect(transport.call('listHostSessions', undefined)).resolves.toEqual([])
  })

  it('keeps completed HTTP watermarks independent across list scopes', async () => {
    const socket = new ControlledWebSocket()
    let socketCreated = false
    const responses = [
      response({ requests: [], next_cursor: null }, 'runtime-a', '7'),
      response({ requests: [], next_cursor: null }, 'runtime-a', '6'),
    ]
    let applicationCalls = 0
    const session = HttpApplicationSession.authenticated({
      accessToken: 'session-token',
      pageUrl: 'https://workbench.example/app',
      fetch: vi.fn<typeof fetch>(async (url) => {
        if (String(url).endsWith('/api/health')) {
          return Response.json({ runtime_generation: 'runtime-a', revision: '5' })
        }
        const next = responses[applicationCalls]
        applicationCalls += 1
        return next!
      }),
      webSocket: () => {
        socketCreated = true
        return socket
      },
    })
    const transport = new HttpApplicationTransport(session.lease())
    await vi.waitFor(() => expect(socketCreated).toBe(true))
    socket.emit({ type: 'ready', runtime_generation: 'runtime-a', revision: '5' })
    const input = (hostSessionId: string) => ({
      host_id: 'codex',
      host_session_id: hostSessionId,
      status: ['waiting' as const, 'in_progress' as const],
      archived: false,
      search: null,
      limit: 50,
      cursor: null,
    })

    await expect(
      transport.call('listFeedbackRequests', input('session-a')),
    ).resolves.toEqual({ requests: [], next_cursor: null })
    await expect(
      transport.call('listFeedbackRequests', input('session-b')),
    ).resolves.toEqual({ requests: [], next_cursor: null })
  })

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

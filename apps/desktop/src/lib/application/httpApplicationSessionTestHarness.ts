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

export class ControlledWebSocket implements ApplicationWebSocket {
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

export function response(
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

export async function flush(): Promise<void> {
  await Promise.resolve()
  await Promise.resolve()
}

export async function expectOlderSemanticProjectionRejected(
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

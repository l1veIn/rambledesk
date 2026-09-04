import { describe, expect, it, vi } from 'vitest'

import { defineApplicationStream } from './applicationTransport'
import type { ApplicationCommandInput, ApplicationCommandName } from './contracts'
import {
  APPLICATION_EVENT_PROTOCOL,
  APPLICATION_EVENTS_STREAM,
  REVISION_HEADER,
  RUNTIME_GENERATION_HEADER,
} from './applicationEvents'
import {
  APPLICATION_CONFORMANCE_INPUTS,
  applicationConformanceResult,
  runApplicationTransportConformance,
} from './applicationTransportConformance'
import {
  HTTP_APPLICATION_OPERATIONS,
  HttpApplicationSession,
  HttpApplicationStreamUnavailableError,
  HttpApplicationTransport,
  StaleHttpApplicationLeaseError,
  applicationCommandProjectionKey,
  applicationCommandResponseResources,
  type ApplicationWebSocket,
} from './httpApplicationTransport'

const TEST_RUNTIME_GENERATION = 'runtime-test'

function readyWebSocket(): ApplicationWebSocket {
  const listeners = new Map<string, Set<EventListener>>()
  const socket = {
    protocol: APPLICATION_EVENT_PROTOCOL,
    readyState: 1,
    addEventListener(type: string, listener: EventListenerOrEventListenerObject) {
      let entries = listeners.get(type)
      if (!entries) {
        entries = new Set()
        listeners.set(type, entries)
      }
      entries.add(listener as EventListener)
    },
    removeEventListener(type: string, listener: EventListenerOrEventListenerObject) {
      listeners.get(type)?.delete(listener as EventListener)
    },
    close() {},
  } as ApplicationWebSocket
  queueMicrotask(() => {
    const event = new MessageEvent('message', {
      data: JSON.stringify({
        type: 'ready',
        runtime_generation: TEST_RUNTIME_GENERATION,
        revision: '0',
      }),
    })
    for (const listener of listeners.get('message') ?? []) listener(event)
  })
  return socket
}

function authenticatedSession(fetchImplementation: typeof fetch) {
  const sessionFetch = vi.fn<typeof fetch>(async (url, init) => {
    if (String(url).endsWith('/api/health')) {
      return Response.json({
        runtime_generation: TEST_RUNTIME_GENERATION,
        revision: '0',
      })
    }
    const response = await fetchImplementation(url, init)
    response.headers.set(RUNTIME_GENERATION_HEADER, TEST_RUNTIME_GENERATION)
    response.headers.set(REVISION_HEADER, '0')
    return response
  })
  return HttpApplicationSession.authenticated({
    accessToken: 'short-lived-session-token',
    pageUrl: 'https://workbench.example/app',
    fetch: sessionFetch,
    webSocket: readyWebSocket,
  })
}

runApplicationTransportConformance('HTTP', () => {
  const calls: Array<readonly [URL | RequestInfo, RequestInit | undefined]> = []
  let rejection: unknown
  const semanticNameByOperation = new Map<string, ApplicationCommandName>(
    Object.entries(HTTP_APPLICATION_OPERATIONS).map(([name, operation]) => [
      operation,
      name as ApplicationCommandName,
    ]),
  )
  const fetchImplementation = vi.fn<typeof fetch>(async (url, init) => {
    calls.push([url, init])
    if (rejection !== undefined) {
      const error = rejection
      rejection = undefined
      return Response.json(error, { status: 409 })
    }
    const operation = String(url).split('/').at(-1) ?? ''
    const name = semanticNameByOperation.get(operation)
    if (!name) return Response.json({ message: 'unknown operation' }, { status: 404 })
    const result = applicationConformanceResult(name)
    if (name === 'cancelAgentInstall' || name === 'deleteHostSession' || name === 'deleteFeedbackRequest' || name === 'deleteAgentConfig' || name === 'deleteManagedSession') {
      return new Response(null, { status: 204 })
    }
    if (result instanceof ArrayBuffer) return new Response(result)
    return Response.json(result)
  })

  return {
    transport: new HttpApplicationTransport(authenticatedSession(fetchImplementation).lease()),
    expectWireCall: async <Name extends ApplicationCommandName>(
      index: number,
      name: Name,
      input: ApplicationCommandInput<Name>,
    ) => {
      const [url, init] = calls[index]!
      expect(String(url).split('/').at(-1)).toBe(HTTP_APPLICATION_OPERATIONS[name])
      expect(init?.method).toBe('POST')
      if (
        name === 'listAvailableAgents' || name === 'listAgentInstallJobs' || name === 'listAgentConfigs' ||
        name === 'listFeedbackInbox' ||
        name === 'listHostSessions' ||
        name === 'listHostProfiles'
      ) {
        expect(init?.body).toBeUndefined()
      } else if (name === 'addFeedbackAttachment') {
        const attachment = input as ApplicationCommandInput<'addFeedbackAttachment'>
        const form = init?.body as FormData
        expect(form.get('request_id')).toBe(attachment.request_id)
        expect(form.get('file_name')).toBe(attachment.file_name)
        expect(form.get('expected_revision')).toBe(String(attachment.expected_revision))
        await expect((form.get('file') as Blob).arrayBuffer()).resolves.toEqual(attachment.contents)
      } else {
        expect(JSON.parse(String(init?.body))).toEqual(input)
      }
    },
    rejectNext: (error) => {
      rejection = error
    },
  }
})

describe('HttpApplicationSession', () => {
  it('requires a base64url short-lived session token', () => {
    expect(() =>
      HttpApplicationSession.authenticated({
        accessToken: 'not a websocket protocol token',
        pageUrl: 'https://workbench.example/app',
        fetch,
      }),
    ).toThrow('base64url session token')
  })

  it('restricts the application endpoint to the Workbench page origin', () => {
    expect(() =>
      HttpApplicationSession.authenticated({
        accessToken: 'session-token',
        pageUrl: 'https://workbench.example/app',
        applicationBaseUrl: 'https://attacker.example/api/application/',
        fetch,
      }),
    ).toThrow('must use the Workbench page origin')
  })

  it('keeps the token in Authorization and out of the request URL', async () => {
    const fetchImplementation = vi.fn<typeof fetch>().mockResolvedValue(Response.json([]))
    const session = authenticatedSession(fetchImplementation)
    const transport = new HttpApplicationTransport(session.lease())

    await transport.call('listFeedbackInbox', undefined)

    const [url, init] = fetchImplementation.mock.calls[0]!
    expect(String(url)).toBe('https://workbench.example/api/application/listFeedbackInbox')
    expect(String(url)).not.toContain('short-lived-session-token')
    expect(new Headers(init?.headers).get('Authorization')).toBe(
      'Bearer short-lived-session-token',
    )
    expect(init).toMatchObject({ credentials: 'same-origin', redirect: 'error' })
    expect(JSON.stringify(session)).not.toContain('short-lived-session-token')
  })

  it('rejects absolute and protocol-relative operations before sending credentials', async () => {
    const fetchImplementation = vi.fn<typeof fetch>()
    const lease = authenticatedSession(fetchImplementation).lease()

    await expect(
      lease.request('https://attacker.example/collect' as never),
    ).rejects.toThrow('Invalid HTTP application operation')
    await expect(lease.request('//attacker.example/collect' as never)).rejects.toThrow(
      'Invalid HTTP application operation',
    )
    expect(fetchImplementation).not.toHaveBeenCalled()
  })

  it('rejects responses that arrive after the authenticated lease becomes stale', async () => {
    let resolveResponse: ((response: Response) => void) | undefined
    const fetchImplementation = vi.fn<typeof fetch>().mockReturnValue(
      new Promise<Response>((resolve) => {
        resolveResponse = resolve
      }),
    )
    const session = authenticatedSession(fetchImplementation)
    const transport = new HttpApplicationTransport(session.lease())
    const pending = transport.call('listHostSessions', undefined)

    session.invalidate()
    resolveResponse?.(Response.json([]))

    await expect(pending).rejects.toBeInstanceOf(StaleHttpApplicationLeaseError)
    await expect(transport.waitUntilReady()).rejects.toBeInstanceOf(
      StaleHttpApplicationLeaseError,
    )
  })
})

describe('HttpApplicationTransport', () => {
  it('deletes the owning managed session with a raw JSON mutation and accepts HTTP 204', async () => {
    const fetchImplementation = vi.fn<typeof fetch>().mockResolvedValue(new Response(null, { status: 204 }))
    const transport = new HttpApplicationTransport(authenticatedSession(fetchImplementation).lease())
    await expect(transport.call('deleteManagedSession', { session_id: 'local-delete' })).resolves.toBeUndefined()
    const [url, init] = fetchImplementation.mock.calls[0]!
    expect(String(url)).toBe('https://workbench.example/api/application/deleteManagedSession')
    expect(JSON.parse(String(init?.body))).toEqual({ session_id: 'local-delete' })
    expect(new Headers(init?.headers).get(RUNTIME_GENERATION_HEADER)).toBe(TEST_RUNTIME_GENERATION)
    expect(fetchImplementation).toHaveBeenCalledTimes(1)
  })
  it.each(['retry', 'acknowledge'] as const)('sends an explicit %s feedback-delivery decision as a managed-session mutation', async (action) => {
    const fetchImplementation = vi.fn<typeof fetch>().mockResolvedValue(Response.json({ accepted: true }))
    const transport = new HttpApplicationTransport(authenticatedSession(fetchImplementation).lease())
    const input = { session_id: 'local-one', request_id: 'feedback-one', action }
    await transport.call('resolveFeedbackDelivery', input)
    const [url, init] = fetchImplementation.mock.calls[0]!
    expect(String(url)).toBe('https://workbench.example/api/application/resolveFeedbackDelivery')
    expect(init?.method).toBe('POST')
    expect(JSON.parse(String(init?.body))).toEqual(input)
    expect(new Headers(init?.headers).get(RUNTIME_GENERATION_HEADER)).toBe(TEST_RUNTIME_GENERATION)
    expect(applicationCommandResponseResources('resolveFeedbackDelivery', input)).toEqual([{ kind: 'managed_session', session_id: 'local-one' }])
    expect(fetchImplementation).toHaveBeenCalledTimes(1)
  })

  it('scopes managed snapshots by local session id and keeps config secrets out of projection keys', () => {
    expect(applicationCommandResponseResources('listAgentConfigs', undefined)).toEqual([{ kind: 'agent_configurations' }])
    expect(applicationCommandResponseResources('getManagedSession', { session_id: 'local-session-one' }))
      .toEqual([{ kind: 'managed_session', session_id: 'local-session-one' }])
    expect(applicationCommandProjectionKey('getManagedSession', { session_id: 'local-session-one' }))
      .not.toBe(applicationCommandProjectionKey('getManagedSession', { session_id: 'local-session-two' }))
    expect(applicationCommandProjectionKey('getManagedSession', { session_id: '0195F7E25C317B5A8AB73C84EA4FC827' }))
      .toBe(applicationCommandProjectionKey('getManagedSession', { session_id: '0195f7e2-5c31-7b5a-8ab7-3c84ea4fc827' }))
    expect(applicationCommandProjectionKey('saveAgentConfig', {
      ...APPLICATION_CONFORMANCE_INPUTS.saveAgentConfig, env: { TOKEN: 'sensitive-value' },
    })).not.toContain('sensitive-value')
    expect(applicationCommandProjectionKey('sendManagedPrompt', {
      session_id: 'local-session-one', text: 'Private prompt content',
    })).not.toContain('Private prompt content')
  })

  it('sends a cancelled permission as explicit null in the authenticated JSON body', async () => {
    const fetchImplementation = vi.fn<typeof fetch>().mockResolvedValue(Response.json({ accepted: true }))
    const transport = new HttpApplicationTransport(authenticatedSession(fetchImplementation).lease())
    await transport.call('respondManagedPermission', {
      session_id: 'local-session-one', request_id: 'permission-one', option_id: null,
    })
    const [url, init] = fetchImplementation.mock.calls[0]!
    expect(String(url)).toBe('https://workbench.example/api/application/respondManagedPermission')
    expect(JSON.parse(String(init?.body))).toEqual({
      session_id: 'local-session-one', request_id: 'permission-one', option_id: null,
    })
    expect(new Headers(init?.headers).get(RUNTIME_GENERATION_HEADER)).toBe(TEST_RUNTIME_GENERATION)
  })

  it('maps command projections to resource-scoped freshness identities', () => {
    expect(applicationCommandResponseResources('listHostSessions', undefined)).toEqual([
      { kind: 'navigation' },
    ])
    expect(
      applicationCommandResponseResources('getFeedbackWorkspace', {
        request_id: 'request-1',
      }),
    ).toEqual([{ kind: 'feedback_workspace', request_id: 'request-1' }])
    expect(
      applicationCommandResponseResources('submitFeedback', {
        request_id: 'request-1',
        expected_revision: 1,
      }),
    ).toEqual([
      { kind: 'feedback_workspace', request_id: 'request-1' },
      { kind: 'published_feedback', request_id: 'request-1' },
    ])
    expect(
      applicationCommandResponseResources('deleteHostSession', {
        host_id: 'codex',
        host_session_id: 'session-1',
      }),
    ).toEqual([
      {
        kind: 'host_session_resources',
        host_id: 'codex',
        host_session_id: 'session-1',
      },
    ])
  })

  it('uses canonical projection identities per operation and list scope', () => {
    const base = {
      host_id: 'codex',
      host_session_id: 'session-1',
      archived: false,
      search: null,
      limit: 50,
      cursor: null,
    }
    const first = applicationCommandProjectionKey('listFeedbackRequests', {
      ...base,
      status: ['waiting', 'completed'],
    })
    const reordered = applicationCommandProjectionKey('listFeedbackRequests', {
      ...base,
      status: ['completed', 'waiting'],
    })
    const otherScope = applicationCommandProjectionKey('listFeedbackRequests', {
      ...base,
      host_session_id: 'session-2',
      status: ['waiting', 'completed'],
    })

    expect(first).toBe(reordered)
    expect(first).not.toBe(otherScope)
    expect(first).not.toBe(
      applicationCommandProjectionKey('listHostSessions', undefined),
    )

    const defaults = applicationCommandProjectionKey('listFeedbackRequests', {
      host_id: null,
      host_session_id: null,
      status: null,
      archived: null,
      search: '   ',
      limit: null,
      cursor: null,
    })
    const explicitDefaults = applicationCommandProjectionKey('listFeedbackRequests', {
      host_id: null,
      host_session_id: null,
      status: ['in_progress', 'waiting', 'waiting'],
      archived: false,
      search: null,
      limit: 50,
      cursor: null,
    })
    expect(defaults).toBe(explicitDefaults)

    expect(
      applicationCommandProjectionKey('listArchivedHostSessions', { search: '  needle  ' }),
    ).toBe(
      applicationCommandProjectionKey('listArchivedHostSessions', { search: 'needle' }),
    )
    expect(
      applicationCommandProjectionKey('listArchivedHostSessions', { search: 'needle' }),
    ).not.toBe(
      applicationCommandProjectionKey('listArchivedHostSessions', { search: 'other' }),
    )
  })

  it('canonicalizes UUID aliases and host session identities for response tracking', () => {
    const canonicalRequestId = '0195f7e2-5c31-7b5a-8ab7-3c84ea4fc827'
    const requestAlias = canonicalRequestId.toUpperCase()
    const canonicalAttachmentId = '0195f7e2-5c31-7b5a-8ab7-3c84ea4fc828'
    const attachmentAlias = canonicalAttachmentId.replaceAll('-', '')

    expect(
      applicationCommandProjectionKey('getFeedbackWorkspace', { request_id: requestAlias }),
    ).toBe(
      applicationCommandProjectionKey('getFeedbackWorkspace', {
        request_id: canonicalRequestId,
      }),
    )
    expect(
      applicationCommandProjectionKey('readFeedbackAttachment', {
        request_id: requestAlias,
        attachment_id: attachmentAlias,
      }),
    ).toBe(
      applicationCommandProjectionKey('readFeedbackAttachment', {
        request_id: canonicalRequestId,
        attachment_id: canonicalAttachmentId,
      }),
    )
    expect(
      applicationCommandResponseResources('getFeedbackWorkspace', {
        request_id: requestAlias,
      }),
    ).toEqual([{ kind: 'feedback_workspace', request_id: canonicalRequestId }])
    expect(
      applicationCommandResponseResources('deleteHostSession', {
        host_id: '  codex ',
        host_session_id: ' session-1  ',
      }),
    ).toEqual([
      {
        kind: 'host_session_resources',
        host_id: 'codex',
        host_session_id: 'session-1',
      },
    ])
  })

  it('defines one complete HTTP operation mapping', () => {
    expect(Object.keys(HTTP_APPLICATION_OPERATIONS)).toHaveLength(42)
    expect(new Set(Object.values(HTTP_APPLICATION_OPERATIONS)).size).toBe(42)
  })

  it('encodes JSON, multipart bytes, binary responses, and no-content outcomes', async () => {
    const fetchImplementation = vi.fn<typeof fetch>(async (url) => {
      const operation = String(url).split('/').at(-1)
      if (operation === 'readFeedbackAttachment') {
        return new Response(new Uint8Array([4, 5, 6]), {
          headers: { 'Content-Type': 'application/octet-stream' },
        })
      }
      if (operation === 'deleteFeedbackRequest') return new Response(null, { status: 204 })
      return Response.json({ ok: true })
    })
    const transport = new HttpApplicationTransport(
      authenticatedSession(fetchImplementation).lease(),
    )

    await transport.call('saveFeedbackDraft', {
      request_id: 'request-1',
      document_json: '{}',
      body_markdown: 'draft',
      expected_revision: 2,
    })
    await transport.call('addFeedbackAttachment', {
      request_id: 'request-1',
      file_name: 'note.txt',
      contents: new Uint8Array([1, 2, 3]).buffer,
      expected_revision: 2,
    })
    await expect(
      transport.call('readFeedbackAttachment', {
        request_id: 'request-1',
        attachment_id: 'attachment-1',
      }),
    ).resolves.toEqual(new Uint8Array([4, 5, 6]).buffer)
    await expect(
      transport.call('deleteFeedbackRequest', { request_id: 'request-1' }),
    ).resolves.toBeUndefined()

    const jsonInit = fetchImplementation.mock.calls[0]![1]!
    expect(new Headers(jsonInit.headers).get('Content-Type')).toBe('application/json')
    expect(JSON.parse(String(jsonInit.body))).toMatchObject({ request_id: 'request-1' })
    const multipart = fetchImplementation.mock.calls[1]![1]!.body
    expect(multipart).toBeInstanceOf(FormData)
    expect((multipart as FormData).get('request_id')).toBe('request-1')
    expect((multipart as FormData).get('expected_revision')).toBe('2')
    await expect(((multipart as FormData).get('file') as Blob).arrayBuffer()).resolves.toEqual(
      new Uint8Array([1, 2, 3]).buffer,
    )
  })

  it('rejects typed ApplicationError responses without normalizing them', async () => {
    const applicationError = {
      code: 'DRAFT_CONFLICT',
      message: 'draft revision changed',
      retryable: false,
    } as const
    const fetchImplementation = vi.fn<typeof fetch>().mockResolvedValue(
      Response.json(applicationError, { status: 409 }),
    )
    const transport = new HttpApplicationTransport(
      authenticatedSession(fetchImplementation).lease(),
    )

    await expect(
      transport.call('saveFeedbackDraft', {
        request_id: 'request-1',
        document_json: '{}',
        body_markdown: 'draft',
        expected_revision: 1,
      }),
    ).rejects.toEqual(applicationError)
  })

  it('preserves retryable unstable snapshot transport errors', async () => {
    const snapshotError = {
      code: 'SNAPSHOT_UNSTABLE',
      message: 'snapshot changed during capture',
      retryable: true,
    } as const
    const transport = new HttpApplicationTransport(
      authenticatedSession(
        vi.fn<typeof fetch>().mockResolvedValue(
          Response.json(snapshotError, { status: 503 }),
        ),
      ).lease(),
    )

    await expect(transport.call('listFeedbackInbox', undefined)).rejects.toEqual(
      snapshotError,
    )
  })

  it('prefers stale-lease rejection when an error body finishes after invalidation', async () => {
    let bodyStarted: (() => void) | undefined
    const readingBody = new Promise<void>((resolve) => {
      bodyStarted = resolve
    })
    let resolveBody: ((body: unknown) => void) | undefined
    const body = new Promise<unknown>((resolve) => {
      resolveBody = resolve
    })
    const response = Response.json({}, { status: 409 })
    vi.spyOn(response, 'json').mockImplementation(async () => {
      bodyStarted?.()
      return body
    })
    const session = authenticatedSession(
      vi.fn<typeof fetch>().mockResolvedValue(response),
    )
    const transport = new HttpApplicationTransport(session.lease())
    const pending = transport.call('saveFeedbackDraft', {
      request_id: 'request-1',
      document_json: '{}',
      body_markdown: 'draft',
      expected_revision: 1,
    })

    await readingBody
    session.invalidate()
    resolveBody?.({ code: 'DRAFT_CONFLICT', message: 'stale error', retryable: false })

    await expect(pending).rejects.toBeInstanceOf(StaleHttpApplicationLeaseError)
  })

  it('prefers stale-lease rejection when successful JSON decoding rejects after invalidation', async () => {
    let decodingStarted: (() => void) | undefined
    const started = new Promise<void>((resolve) => {
      decodingStarted = resolve
    })
    let rejectDecode: ((cause: unknown) => void) | undefined
    const decoding = new Promise<unknown>((_resolve, reject) => {
      rejectDecode = reject
    })
    const response = Response.json({})
    vi.spyOn(response, 'json').mockImplementation(async () => {
      decodingStarted?.()
      return decoding
    })
    const session = authenticatedSession(vi.fn<typeof fetch>().mockResolvedValue(response))
    const transport = new HttpApplicationTransport(session.lease())
    const pending = transport.call('listFeedbackInbox', undefined)

    await started
    session.invalidate()
    rejectDecode?.(new SyntaxError('invalid JSON'))

    await expect(pending).rejects.toBeInstanceOf(StaleHttpApplicationLeaseError)
  })

  it('prefers stale-lease rejection when successful binary decoding rejects after invalidation', async () => {
    let decodingStarted: (() => void) | undefined
    const started = new Promise<void>((resolve) => {
      decodingStarted = resolve
    })
    let rejectDecode: ((cause: unknown) => void) | undefined
    const decoding = new Promise<ArrayBuffer>((_resolve, reject) => {
      rejectDecode = reject
    })
    const response = new Response(new Uint8Array([1, 2, 3]))
    vi.spyOn(response, 'arrayBuffer').mockImplementation(async () => {
      decodingStarted?.()
      return decoding
    })
    const session = authenticatedSession(vi.fn<typeof fetch>().mockResolvedValue(response))
    const transport = new HttpApplicationTransport(session.lease())
    const pending = transport.call('readFeedbackAttachment', {
      request_id: 'request-1',
      attachment_id: 'attachment-1',
    })

    await started
    session.invalidate()
    rejectDecode?.(new TypeError('binary stream failed'))

    await expect(pending).rejects.toBeInstanceOf(StaleHttpApplicationLeaseError)
  })

  it('reports streams as unavailable and only marks an active authenticated lease ready', async () => {
    const session = authenticatedSession(vi.fn<typeof fetch>())
    const transport = new HttpApplicationTransport(session.lease())
    await expect(transport.waitUntilReady()).resolves.toBeUndefined()

    const onError = vi.fn()
    const unsubscribe = transport.subscribe(
      defineApplicationStream('request:changed'),
      vi.fn(),
      onError,
    )
    await Promise.resolve()
    expect(onError).toHaveBeenCalledWith(expect.any(HttpApplicationStreamUnavailableError))
    unsubscribe()
    unsubscribe()
  })

  it('does not report ready before its lease readiness barrier resolves', async () => {
    let releaseReady: (() => void) | undefined
    const ready = new Promise<void>((resolve) => {
      releaseReady = resolve
    })
    const transport = new HttpApplicationTransport({
      request: () => Promise.reject(new Error('not used')),
      assertActive: () => undefined,
      waitUntilReady: () => ready,
      subscribe: () => () => undefined,
    })
    let settled = false
    const waiting = transport.waitUntilReady().then(() => {
      settled = true
    })

    await Promise.resolve()
    expect(settled).toBe(false)
    releaseReady?.()
    await waiting
    expect(settled).toBe(true)
  })
})

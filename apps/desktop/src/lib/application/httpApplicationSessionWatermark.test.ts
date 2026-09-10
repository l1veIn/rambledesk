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

describe('HttpApplicationSession projection watermarks', () => {
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

})

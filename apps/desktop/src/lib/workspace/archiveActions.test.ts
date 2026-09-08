import { get } from 'svelte/store'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
import type { HostSessionSummary } from '$lib/feedback'
import { previewFixtures } from '$lib/previewFixtures'
import { createArchiveActions, type ArchiveActionsContext } from './archiveActions'

const archivedSession = previewFixtures.archivedHostSessions[0]
const request = previewFixtures.requests[0]

function harness(
  overrides: Partial<ArchiveActionsContext> = {},
  workspaceFixture = previewFixtures.workspace,
) {
  // The handlers differ in result type; each call site casts through `any`.
  const calls = vi.fn(async (name: string, _input?: unknown): Promise<any> => {
    if (name === 'listArchivedHostSessions') return [archivedSession]
    if (name === 'listFeedbackRequests') return { requests: [request], next_cursor: null }
    if (name === 'getFeedbackWorkspace') return workspaceFixture
    if (name === 'readPublishedFeedback') return { markdown: '# Published' }
    return undefined
  })
  const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
    .handle('listArchivedHostSessions', (input) => calls('listArchivedHostSessions', input))
    .handle('listFeedbackRequests', (input) => calls('listFeedbackRequests', input))
    .handle('getFeedbackWorkspace', (input) => calls('getFeedbackWorkspace', input))
    .handle('readPublishedFeedback', (input) => calls('readPublishedFeedback', input))
    .handle('unarchiveHostSession', (input) => calls('unarchiveHostSession', input))
    .handle('deleteHostSession', (input) => calls('deleteHostSession', input))
    .handle('deleteFeedbackRequest', (input) => calls('deleteFeedbackRequest', input))
  const context = {
    transport,
    isPreviewMode: () => false,
    tr: (source: string) => source,
    messageFrom: (cause: unknown) => String(cause),
    onError: vi.fn(),
    onChanged: vi.fn(async () => undefined),
    reload: vi.fn(async () => undefined),
    ...overrides,
  } as ArchiveActionsContext
  return { actions: createArchiveActions(context), context, calls, transport }
}

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('archive actions', () => {
  it('loads archived sessions and their requests, then clears the loading flag', async () => {
    const { actions, calls } = harness()

    const loaded = await actions.load('  ')

    expect(loaded?.sessions).toEqual([archivedSession])
    expect(calls).toHaveBeenCalledWith('listArchivedHostSessions', { search: null })
    expect(calls).toHaveBeenCalledWith(
      'listFeedbackRequests',
      expect.objectContaining({ archived: true, search: null }),
    )
    expect(get(actions).loading).toBe(false)
    expect(get(actions).requestsBySession[Object.keys(get(actions).requestsBySession)[0]!]).toEqual([
      request,
    ])
  })

  it('reports a load failure without replacing the previous archive', async () => {
    const onError = vi.fn()
    const { actions, transport } = harness({ onError })
    transport.reject('listArchivedHostSessions', new Error('archive unavailable'))

    await expect(actions.load('')).resolves.toBeNull()

    expect(onError).toHaveBeenCalledWith('Error: archive unavailable')
    expect(get(actions)).toMatchObject({ loading: false, sessions: [] })
  })

  it('loads request details once and reuses them afterwards', async () => {
    const completed = { ...request, status: 'completed' as const }
    const { actions, calls } = harness({}, {
      ...previewFixtures.workspace,
      request: { ...previewFixtures.workspace.request, status: 'completed' },
      feedback: { markdown: '# Cooked' } as never,
    })

    await actions.loadRequestDetails(completed)
    await actions.loadRequestDetails(completed)

    expect(calls).toHaveBeenCalledTimes(2)
    expect(get(actions).requestDetailsById[completed.request_id]?.publishedFeedback).toEqual({
      markdown: '# Published',
      uncooked_markdown: undefined,
    })
    expect(get(actions).detailLoadingRequestId).toBeNull()
  })

  it('unarchives a session and reloads the view', async () => {
    const { actions, calls, context } = harness()

    await actions.unarchiveSession(archivedSession)

    expect(calls).toHaveBeenCalledWith('unarchiveHostSession', {
      host_id: archivedSession.host_id,
      host_session_id: archivedSession.host_session_id,
    })
    expect(context.onChanged).toHaveBeenCalled()
    expect(context.reload).toHaveBeenCalled()
    expect(get(actions).busyKey).toBeNull()
  })

  it('keeps the confirmation guard for destructive deletes', async () => {
    const confirmSpy = vi.fn(() => false)
    vi.stubGlobal('confirm', confirmSpy)
    const { actions, calls } = harness()

    await actions.deleteRequest(request)

    expect(confirmSpy).toHaveBeenCalled()
    expect(calls).not.toHaveBeenCalledWith('deleteFeedbackRequest', expect.anything())
    expect(get(actions).busyKey).toBeNull()
  })

  it('routes managed session deletion through the caller and unmanaged deletion through the transport', async () => {
    vi.stubGlobal('confirm', vi.fn(() => true))
    const onDeleteManagedSession = vi.fn(async () => undefined)
    const { actions, calls } = harness({ onDeleteManagedSession })
    const managed = {
      ...archivedSession,
      management: {
        kind: 'managed',
        protocol: 'acp',
        agent_config_id: 'config',
        cwd: '/repo',
        remote_session_id: 'remote',
      },
    } as HostSessionSummary

    await actions.deleteSession(managed)
    expect(onDeleteManagedSession).toHaveBeenCalledWith(managed)

    await actions.deleteSession(archivedSession)
    expect(calls).toHaveBeenCalledWith('deleteHostSession', {
      host_id: archivedSession.host_id,
      host_session_id: archivedSession.host_session_id,
    })
  })
})

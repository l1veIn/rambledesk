import { describe, expect, it } from 'vitest'

import { UNAVAILABLE_CAPABILITY_MANIFEST } from '../capabilities/unavailableCapabilities'
import type { ApplicationCommandName } from '../application/contracts'
import { PreviewApplicationTransport } from './previewApplicationTransport'
import { previewFixtures } from './previewFixtures'

function transport() {
  return new PreviewApplicationTransport(UNAVAILABLE_CAPABILITY_MANIFEST)
}

function call(name: ApplicationCommandName, input?: unknown) {
  return transport().call(name as never, input as never) as unknown as Promise<unknown>
}

describe('preview application transport', () => {
  it('answers the navigation reads from the fixtures', async () => {
    const preview = transport()

    const inbox = (await preview.call('listFeedbackInbox', undefined)) as Array<{
      status: string
    }>
    expect(inbox.map((request) => request.status).sort()).toEqual(['in_progress', 'waiting'])
    expect(await preview.call('listHostSessions', undefined)).toEqual(
      previewFixtures.hostSessions,
    )
    expect(await preview.call('listHostProfiles', undefined)).toEqual(
      previewFixtures.hostProfiles,
    )
    expect(await preview.waitUntilReady()).toBeUndefined()
    expect(preview.capabilities()).toBe(UNAVAILABLE_CAPABILITY_MANIFEST)
  })

  it('applies scope, status, and search filters like the server', async () => {
    const preview = transport()
    const scope = {
      host_id: 'codex',
      host_session_id: 'desktop-refactor-2026-08-02',
      archived: null,
      limit: 100,
      cursor: null,
    }

    const scoped = (await preview.call('listFeedbackRequests', {
      ...scope,
      status: ['waiting', 'in_progress'],
      search: null,
    })) as { requests: Array<{ status: string }> }
    expect(scoped.requests.map((request) => request.status).sort()).toEqual([
      'in_progress',
      'waiting',
    ])

    const searched = (await preview.call('listFeedbackRequests', {
      ...scope,
      status: ['waiting', 'in_progress', 'completed', 'cancelled'],
      search: 'adapter',
    })) as { requests: Array<{ request_id: string }> }
    expect(searched.requests).toHaveLength(1)
    expect(searched.requests[0]?.request_id).toBe('019fc1d9-51e7-7eb2-b196-e9266947fc42')

    const empty = (await preview.call('listFeedbackRequests', {
      ...scope,
      status: ['cancelled'],
      search: null,
    })) as { requests: unknown[] }
    expect(empty.requests).toEqual([])
  })

  it('paginates with the cursor', async () => {
    const preview = transport()
    const first = (await preview.call('listFeedbackRequests', {
      host_id: null,
      host_session_id: null,
      status: ['waiting', 'in_progress', 'completed', 'cancelled'],
      archived: false,
      search: null,
      limit: 2,
      cursor: null,
    })) as { requests: unknown[]; next_cursor: string | null }
    expect(first.requests).toHaveLength(2)
    expect(first.next_cursor).toBe('2')

    const second = (await preview.call('listFeedbackRequests', {
      host_id: null,
      host_session_id: null,
      status: ['waiting', 'in_progress', 'completed', 'cancelled'],
      archived: false,
      search: null,
      limit: 2,
      cursor: first.next_cursor,
    })) as { requests: unknown[]; next_cursor: string | null }
    expect(second.requests).toHaveLength(2)
    expect(second.next_cursor).toBeNull()
  })

  it('serves archived sessions and keeps them out of the active list', async () => {
    const preview = transport()
    const archived = (await preview.call('listArchivedHostSessions', { search: null })) as Array<{
      session_id: string
    }>
    expect(archived).toHaveLength(previewFixtures.archivedHostSessions.length)

    const filtered = (await preview.call('listArchivedHostSessions', {
      search: archived[0]!.session_id.slice(0, 6),
    })) as unknown[]
    expect(filtered.length).toBeGreaterThan(0)
  })

  it('persists a draft save and exposes it on the next workspace read', async () => {
    const preview = transport()
    const requestId = previewFixtures.requests[0]!.request_id

    const saved = (await preview.call('saveFeedbackDraft', {
      request_id: requestId,
      document_json: '{"type":"doc"}',
      body_markdown: 'Edited in preview',
      expected_revision: 3,
    })) as { body_markdown: string; saved_revision: number }
    expect(saved).toMatchObject({ body_markdown: 'Edited in preview', saved_revision: 4 })

    const workspace = (await preview.call('getFeedbackWorkspace', { request_id: requestId })) as {
      draft: { body_markdown: string; saved_revision: number }
    }
    expect(workspace.draft).toMatchObject({
      body_markdown: 'Edited in preview',
      saved_revision: 4,
    })
  })

  it('marks a submitted request completed and publishes a package view', async () => {
    const preview = transport()
    const requestId = previewFixtures.requests[0]!.request_id

    const submitted = (await preview.call('submitFeedback', {
      request_id: requestId,
      expected_revision: 3,
    })) as { status: string; feedback: { available: boolean } }
    expect(submitted.status).toBe('completed')
    expect(submitted.feedback).toEqual({ available: true })

    const published = (await preview.call('readPublishedFeedback', {
      request_id: requestId,
    })) as { markdown: string } | null
    expect(published?.markdown).toContain('host and session hierarchy')
  })

  it('mutates host sessions for rename, pin, archive, unarchive, and delete', async () => {
    const preview = transport()
    const session = previewFixtures.hostSessions[0]!

    const renamed = (await preview.call('renameHostSession', {
      host_id: session.host_id,
      host_session_id: session.host_session_id,
      title: 'Renamed in preview',
    })) as { title: string }
    expect(renamed.title).toBe('Renamed in preview')

    const pinned = (await preview.call('setHostSessionPinned', {
      host_id: session.host_id,
      host_session_id: session.host_session_id,
      pinned: true,
    })) as { pinned_at: string | null }
    expect(pinned.pinned_at).not.toBeNull()

    const archived = (await preview.call('archiveHostSession', {
      host_id: session.host_id,
      host_session_id: session.host_session_id,
    })) as { archived_at: string | null }
    expect(archived.archived_at).not.toBeNull()
    const remaining = (await preview.call('listHostSessions', undefined)) as unknown[]
    expect(remaining).toHaveLength(previewFixtures.hostSessions.length - 1)

    const restored = (await preview.call('unarchiveHostSession', {
      host_id: session.host_id,
      host_session_id: session.host_session_id,
    })) as { archived_at: string | null }
    expect(restored.archived_at).toBeNull()

    await preview.call('deleteHostSession', {
      host_id: session.host_id,
      host_session_id: session.host_session_id,
    })
    const afterDelete = (await preview.call('listHostSessions', undefined)) as unknown[]
    expect(afterDelete).toHaveLength(previewFixtures.hostSessions.length - 1)
  })

  it('fails loudly for commands the fixture server does not model', async () => {
    await expect(call('getManagedSession', { session_id: 'session-1' })).rejects.toThrow(
      'The preview transport does not implement getManagedSession.',
    )
  })
})

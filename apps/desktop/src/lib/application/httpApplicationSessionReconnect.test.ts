import { describe, expect, it, vi } from 'vitest'
import { get } from 'svelte/store'

import { snapshotFeedbackDraftDocument } from '../feedbackDraftDocument'
import { previewFixtures } from '../preview/previewFixtures'
import { createDraftController } from '../workbench/draftController'
import { createDraftSession } from '../workbench/draftSession'
import { createWorkspaceSession } from '../workbench/workspaceSession'
import { replaceReadyApplicationTransport } from './browserReauthentication'
import {
  HttpApplicationSession,
  HttpApplicationSessionRevokedError,
  HttpApplicationTransport,
  StaleHttpApplicationLeaseError,
} from './httpApplicationTransport'
import { ControlledWebSocket, response } from './httpApplicationSessionTestHarness'
import { ReplaceableApplicationTransport } from './replaceableApplicationTransport'

function snapshot(text: string) {
  return snapshotFeedbackDraftDocument({
    type: 'doc',
    content: [{ type: 'paragraph', content: [{ type: 'text', text }] }],
  })
}

const offlineDraft = snapshot('OFFLINE DRAFT')
const draft = {
  request_id: 'request-1',
  document_json: offlineDraft.documentJson,
  body_markdown: offlineDraft.bodyMarkdown,
  expected_revision: 2,
}

async function connected() {
  let health: () => Promise<Response> = async () =>
    Response.json({ runtime_generation: 'runtime-a', revision: '2' })
  const sockets: ControlledWebSocket[] = []
  const reconnects: Array<() => void> = []
  const onTerminalError = vi.fn()
  const applicationCalls = vi.fn(async () => response({
    document_json: draft.document_json,
    body_markdown: draft.body_markdown,
    saved_revision: 3,
    updated_at: null,
  }, 'runtime-a', '3'))
  const session = HttpApplicationSession.authenticated({
    accessToken: 'test-session',
    pageUrl: 'https://workbench.example/app',
    fetch: async (url) => String(url).endsWith('/api/health')
      ? health() : applicationCalls(),
    webSocket: () => {
      const socket = new ControlledWebSocket()
      sockets.push(socket)
      return socket
    },
    scheduleReconnect: (callback) => {
      reconnects.push(callback)
      return () => undefined
    },
    onTerminalError,
  })
  const transport = new HttpApplicationTransport(session.lease())
  await vi.waitFor(() => expect(sockets).toHaveLength(1))
  sockets[0]!.emit({ type: 'ready', runtime_generation: 'runtime-a', revision: '2' })
  await transport.waitUntilReady()
  return { session, transport, sockets, reconnects, applicationCalls, onTerminalError,
    setHealth: (next: typeof health) => { health = next } }
}

describe('HTTP commands queued between disconnect and reconnect', () => {
  it('settles the existing readiness wait and sends a queued save once after reconnect ready', async () => {
    const fixture = await connected()
    try {
      fixture.sockets[0]!.disconnect()
      const ready = vi.fn()
      const waiting = fixture.transport.waitUntilReady().then(ready)
      const saved = vi.fn()
      const saving = fixture.transport.call('saveFeedbackDraft', draft).then(saved)
      expect(fixture.applicationCalls).not.toHaveBeenCalled()

      fixture.reconnects[0]!()
      await vi.waitFor(() => expect(fixture.sockets).toHaveLength(2))
      fixture.sockets[1]!.emit({ type: 'ready', runtime_generation: 'runtime-a', revision: '2' })

      await vi.waitFor(() => expect(saved).toHaveBeenCalledOnce(), { timeout: 200 })
      await Promise.all([waiting, saving])
      expect(ready).toHaveBeenCalledOnce()
      expect(fixture.applicationCalls).toHaveBeenCalledOnce()
      expect(saved).toHaveBeenCalledWith(expect.objectContaining({ saved_revision: 3 }))
    } finally {
      fixture.session.invalidate()
    }
  })

  it('settles an old queued save when reconnect discovers revocation, permitting retry after reauthentication', async () => {
    const fixture = await connected()
    const current = new ReplaceableApplicationTransport(fixture.transport)
    let next: Awaited<ReturnType<typeof connected>> | undefined
    try {
      fixture.sockets[0]!.disconnect()
      const rejected = vi.fn()
      const saving = current.call('saveFeedbackDraft', draft).catch(rejected)
      fixture.setHealth(async () => new Response(null, { status: 401 }))
      fixture.reconnects[0]!()
      await vi.waitFor(() => expect(fixture.onTerminalError).toHaveBeenCalledOnce())

      next = await connected()
      await replaceReadyApplicationTransport(current, next.transport, () => undefined)
      await vi.waitFor(() => expect(rejected).toHaveBeenCalledOnce(), { timeout: 200 })
      await saving
      expect(rejected).toHaveBeenCalledWith(expect.any(HttpApplicationSessionRevokedError))
      expect(fixture.applicationCalls).not.toHaveBeenCalled()
      expect(next.applicationCalls).not.toHaveBeenCalled()
      await expect(current.call('saveFeedbackDraft', draft)).resolves.toMatchObject({ saved_revision: 3 })
      expect(next.applicationCalls).toHaveBeenCalledOnce()
    } finally {
      fixture.session.invalidate()
      next?.session.invalidate()
    }
  })

  it('settles a queued save if the next health probe fails before readiness', async () => {
    const fixture = await connected()
    try {
      fixture.sockets[0]!.disconnect()
      const rejected = vi.fn()
      const saving = fixture.transport.call('saveFeedbackDraft', draft).catch(rejected)
      fixture.setHealth(async () => { throw new TypeError('network unavailable') })
      fixture.reconnects[0]!()
      await vi.waitFor(() => expect(fixture.reconnects).toHaveLength(2))

      await vi.waitFor(() => expect(rejected).toHaveBeenCalledOnce(), { timeout: 200 })
      await saving
      expect(rejected).toHaveBeenCalledWith(expect.any(StaleHttpApplicationLeaseError))
      expect(fixture.applicationCalls).not.toHaveBeenCalled()
    } finally {
      fixture.session.invalidate()
    }
  })

  it('releases the actual draft save flight and saves retained edits through the reauthenticated save gate', async () => {
    const first = await connected()
    const current = new ReplaceableApplicationTransport(first.transport)
    const session = createDraftSession()
    const workspace = createWorkspaceSession()
    const original = snapshot('SAVED DRAFT')
    const serverDraft = {
      document_json: original.documentJson, body_markdown: original.bodyMarkdown,
      saved_revision: 2, updated_at: '2026-09-10T00:00:00Z',
    }
    session.adopt(serverDraft)
    workspace.open({
      ...previewFixtures.workspace,
      request: { ...previewFixtures.workspace.request, request_id: draft.request_id },
      draft: serverDraft,
    })
    const controller = createDraftController({
      transport: current, session,
      getWorkspace: () => get(workspace).workspace,
      setWorkspaceDraft: workspace.setDraft,
      isInteractionLocked: () => false,
      isWorkspaceTerminal: workspace.isTerminal,
      messageFrom: (cause) => cause instanceof Error ? cause.message : String(cause),
    })
    let next: Awaited<ReturnType<typeof connected>> | undefined
    try {
      first.sockets[0]!.disconnect()
      controller.updateDraft(offlineDraft)
      const saving = controller.saveDraftNow()
      await vi.waitFor(() => expect(get(session).phase).toBe('saving'))
      expect(get(workspace).workspace?.draft.saved_revision).toBe(2)
      first.setHealth(async () => new Response(null, { status: 401 }))
      first.reconnects[0]!()
      await vi.waitFor(() => expect(get(session).phase).toBe('error'), { timeout: 200 })
      expect(await saving).toBe(false)
      expect(controller.hasPendingSave()).toBe(false)
      expect(session.snapshot()).toEqual(offlineDraft)
      expect(get(session).savedRevision).toBe(2)
      const editorEpoch = get(session).editorEpoch

      next = await connected()
      let retry: Promise<boolean> | undefined
      await replaceReadyApplicationTransport(current, next.transport, () => {
        retry = controller.saveDraftNow()
      })
      expect(await retry).toBe(true)
      expect(get(session)).toMatchObject({ phase: 'saved', savedRevision: 3, dirty: false, editorEpoch })
      expect(session.snapshot()).toEqual(offlineDraft)
      expect(get(workspace).workspace?.draft).toMatchObject({ saved_revision: 3, body_markdown: 'OFFLINE DRAFT' })
      expect(first.applicationCalls).not.toHaveBeenCalled()
      expect(next.applicationCalls).toHaveBeenCalledOnce()
    } finally {
      controller.cancelPendingSave()
      first.session.invalidate()
      next?.session.invalidate()
    }
  })
})

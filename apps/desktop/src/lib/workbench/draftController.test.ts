import { get } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { TestApplicationTransport } from '../application/testApplicationTransport'
import type { DraftView, SaveDraftInput } from '../feedback'
import { snapshotFeedbackDraftDocument } from '../feedbackDraftDocument'
import { previewFixtures } from '../preview/previewFixtures'
import { createDraftController } from './draftController'
import { createDraftSession } from './draftSession'
import { createWorkspaceSession } from './workspaceSession'

function snapshot(text: string) {
  return snapshotFeedbackDraftDocument({
    type: 'doc',
    content: [{ type: 'paragraph', content: [{ type: 'text', text }] }],
  })
}

function draft(text: string, revision: number): DraftView {
  const value = snapshot(text)
  return {
    document_json: value.documentJson,
    body_markdown: value.bodyMarkdown,
    saved_revision: revision,
    updated_at: '2026-09-09T00:00:00Z',
  }
}

function harness() {
  const session = createDraftSession()
  const workspace = createWorkspaceSession()
  const transport = new TestApplicationTransport()
  const saves: Array<{
    input: SaveDraftInput
    accept: (revision: number) => void
    reject: (cause: Error) => void
  }> = []
  transport.handle('saveFeedbackDraft', (input) => new Promise<DraftView>((resolve, reject) => {
    saves.push({
      input,
      accept: (revision) => resolve({
        document_json: input.document_json,
        body_markdown: input.body_markdown,
        saved_revision: revision,
        updated_at: '2026-09-09T00:00:01Z',
      }),
      reject,
    })
  }))
  function open(requestId: string, text: string, revision: number) {
    const serverDraft = draft(text, revision)
    workspace.open({
      ...previewFixtures.workspace,
      request: { ...previewFixtures.workspace.request, request_id: requestId },
      draft: serverDraft,
    })
    session.adopt(serverDraft)
  }
  open('request-a', 'original', 3)
  const controller = createDraftController({
    transport,
    session,
    getWorkspace: () => get(workspace).workspace,
    setWorkspaceDraft: workspace.setDraft,
    isInteractionLocked: () => get(workspace).interactionLocked,
    isWorkspaceTerminal: workspace.isTerminal,
    messageFrom: (cause) => cause instanceof Error ? cause.message : String(cause),
  })
  return { controller, session, workspace, transport, saves, open }
}

beforeEach(() => vi.useFakeTimers())
afterEach(() => vi.useRealTimers())

describe('draft controller persistence', () => {
  it('updates the real editing session synchronously and debounces the latest input', async () => {
    const { controller, session, saves } = harness()
    controller.updateDraft(snapshot('first'))
    expect(session.snapshot()).toEqual(snapshot('first'))
    expect(get(session).phase).toBe('unsaved')
    await vi.advanceTimersByTimeAsync(699)
    expect(saves).toHaveLength(0)

    controller.updateDraft(snapshot('latest'))
    await vi.advanceTimersByTimeAsync(699)
    expect(saves).toHaveLength(0)
    await vi.advanceTimersByTimeAsync(1)
    expect(saves).toHaveLength(1)
    expect(saves[0]!.input).toMatchObject({ body_markdown: 'latest', expected_revision: 3 })
    saves[0]!.accept(4)
    await vi.advanceTimersByTimeAsync(0)
    expect(session.isDirty()).toBe(false)
  })

  it('waits for an in-flight edit even when the user has reverted to the old saved document', async () => {
    const { controller, session, saves } = harness()
    const original = session.savedSnapshot()
    controller.updateDraft(snapshot('in flight'))
    const first = controller.saveDraftNow()
    await vi.advanceTimersByTimeAsync(0)
    controller.updateDraft(original)
    expect(session.isDirty()).toBe(false)
    let settled = false
    const waiting = controller.saveDraftNow().then((saved) => { settled = true; return saved })
    await vi.advanceTimersByTimeAsync(0)
    expect(settled).toBe(false)

    saves[0]!.accept(4)
    await vi.advanceTimersByTimeAsync(0)
    expect(saves).toHaveLength(2)
    expect(saves[1]!.input).toMatchObject({ body_markdown: 'original', expected_revision: 4 })
    expect(settled).toBe(false)
    saves[1]!.accept(5)
    expect(await Promise.all([first, waiting])).toEqual([true, true])
    expect(get(session)).toMatchObject({ savedRevision: 5, phase: 'saved' })
    expect(session.savedSnapshot()).toEqual(original)
  })

  it('holds all save waiters until subsequent edits are saved with the latest server revision', async () => {
    const { controller, session, workspace, saves } = harness()
    controller.updateDraft(snapshot('first'))
    const first = controller.saveDraftNow()
    await vi.advanceTimersByTimeAsync(0)
    controller.updateDraft(snapshot('second'))
    let settled = false
    const waiting = controller.saveDraftNow().then((saved) => { settled = true; return saved })
    saves[0]!.accept(10)
    await vi.advanceTimersByTimeAsync(0)
    expect(saves).toHaveLength(2)
    expect(saves[1]!.input).toMatchObject({ body_markdown: 'second', expected_revision: 10 })
    expect(settled).toBe(false)
    controller.updateDraft(snapshot('latest'))
    saves[1]!.accept(11)
    await vi.advanceTimersByTimeAsync(0)
    expect(saves).toHaveLength(3)
    expect(saves[2]!.input).toMatchObject({ body_markdown: 'latest', expected_revision: 11 })
    expect(settled).toBe(false)
    saves[2]!.accept(12)

    expect(await Promise.all([first, waiting])).toEqual([true, true])
    expect(session.savedSnapshot()).toEqual(snapshot('latest'))
    expect(get(workspace).workspace?.draft).toMatchObject({ body_markdown: 'latest', saved_revision: 12 })
    await vi.advanceTimersByTimeAsync(700)
    expect(saves).toHaveLength(3)
  })

  it('shares a CAS failure with every waiter without retrying, then permits an explicit retry', async () => {
    const { controller, session, saves } = harness()
    controller.updateDraft(snapshot('first'))
    const first = controller.saveDraftNow()
    await vi.advanceTimersByTimeAsync(0)
    const waiting = controller.saveDraftNow()
    // This edit also schedules a debounce that must not retry the failed batch.
    controller.updateDraft(snapshot('latest'))
    saves[0]!.reject(new Error('revision conflict'))
    await vi.advanceTimersByTimeAsync(0)
    expect(saves).toHaveLength(1)
    expect(await Promise.all([first, waiting])).toEqual([false, false])
    expect(get(session)).toMatchObject({ phase: 'error', message: 'revision conflict', savedRevision: 3 })
    expect(session.snapshot()).toEqual(snapshot('latest'))
    await vi.advanceTimersByTimeAsync(700)
    expect(saves).toHaveLength(1)

    const retry = controller.saveDraftNow()
    await vi.advanceTimersByTimeAsync(0)
    expect(saves[1]!.input).toMatchObject({ body_markdown: 'latest', expected_revision: 3 })
    saves[1]!.accept(4)
    expect(await retry).toBe(true)
    expect(get(session)).toMatchObject({ phase: 'saved', message: '', savedRevision: 4 })
  })

  it('does not attach a new save to a drain that has already finished accepting its result', async () => {
    const { controller, session, saves } = harness()
    controller.updateDraft(snapshot('first'))
    const first = controller.saveDraftNow()
    await vi.advanceTimersByTimeAsync(0)
    let nextSave: Promise<boolean> | undefined
    let nextSettled = false
    const unsubscribe = session.subscribe((state) => {
      if (state.savedRevision !== 4 || state.phase !== 'saved') return
      // The next explicit save runs after the drain's final dirty check, but
      // before callers awaiting its promise have necessarily resumed.
      void Promise.resolve().then(() => {
        controller.updateDraft(snapshot('next'))
        nextSave = controller.saveDraftNow().then((saved) => { nextSettled = true; return saved })
      })
    })
    saves[0]!.accept(4)
    await vi.advanceTimersByTimeAsync(0)
    unsubscribe()
    expect(await first).toBe(true)
    expect(nextSettled).toBe(false)
    expect(saves).toHaveLength(2)
    expect(saves[1]!.input).toMatchObject({ body_markdown: 'next', expected_revision: 4 })
    saves[1]!.accept(5)
    expect(await nextSave).toBe(true)
  })

  it('does not apply a late failure or cancel autosave for a different request', async () => {
    const { controller, session, workspace, saves, open } = harness()
    controller.updateDraft(snapshot('old request edit'))
    const saving = controller.saveDraftNow()
    await vi.advanceTimersByTimeAsync(0)
    open('request-b', 'other request', 7)
    controller.updateDraft(snapshot('other request edited'))
    const currentState = get(session)
    saves[0]!.reject(new Error('old request failed'))

    expect(await saving).toBe(false)
    expect(get(session)).toEqual(currentState)
    expect(get(workspace).workspace?.draft).toEqual(draft('other request', 7))
    await vi.advanceTimersByTimeAsync(700)
    expect(saves).toHaveLength(2)
    expect(saves[1]!.input).toMatchObject({ request_id: 'request-b', expected_revision: 7 })
    saves[1]!.accept(8)
    await vi.advanceTimersByTimeAsync(0)
    expect(session.savedSnapshot()).toEqual(snapshot('other request edited'))
  })

  it('ignores an old request response and drains the newly open request using its own revision', async () => {
    const { controller, session, workspace, saves, open } = harness()
    controller.updateDraft(snapshot('old request edit'))
    const saving = controller.saveDraftNow()
    await vi.advanceTimersByTimeAsync(0)
    open('request-b', 'other request', 7)
    controller.updateDraft(snapshot('new request edit'))
    const waiting = controller.saveDraftNow()
    saves[0]!.accept(4)
    await vi.advanceTimersByTimeAsync(0)
    expect(get(session).savedRevision).toBe(7)
    expect(get(workspace).workspace?.draft).toEqual(draft('other request', 7))
    expect(saves[1]!.input).toMatchObject({
      request_id: 'request-b', body_markdown: 'new request edit', expected_revision: 7,
    })
    saves[1]!.accept(8)
    expect(await Promise.all([saving, waiting])).toEqual([true, true])
    expect(get(session)).toMatchObject({ savedRevision: 8, phase: 'saved' })
    expect(get(workspace).workspace?.draft.body_markdown).toBe('new request edit')
  })

  it('ignores editor input while locked, terminal or without a workspace', async () => {
    const { controller, session, workspace, saves } = harness()
    const original = session.snapshot()
    workspace.beginApprove()
    controller.updateDraft(snapshot('locked'))
    workspace.endApprove()
    workspace.replace({
      ...get(workspace).workspace!,
      request: { ...workspace.request()!, status: 'completed' },
    })
    controller.updateDraft(snapshot('terminal'))
    workspace.close()
    controller.updateDraft(snapshot('closed'))
    await vi.advanceTimersByTimeAsync(700)
    expect(session.snapshot()).toEqual(original)
    expect(saves).toHaveLength(0)
    expect(await controller.saveDraftNow()).toBe(true)
  })
})

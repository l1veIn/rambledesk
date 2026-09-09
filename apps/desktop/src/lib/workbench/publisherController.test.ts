import { get } from 'svelte/store'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { DraftView, FeedbackRequestView, FeedbackWorkspaceView } from '../feedback'
import { snapshotFeedbackDraftDocument } from '../feedbackDraftDocument'
import { TestApplicationTransport } from '../application/testApplicationTransport'
import { StaleHttpApplicationResponseError } from '../application/httpApplicationTransport'
import { createCookingSession } from './cookingSession'
import { createDraftController } from './draftController'
import { createDraftSession } from './draftSession'
import { createPublisherController } from './publisherController'
import { createWorkspaceSession } from './workspaceSession'

const originalBody = 'Original uncooked ramble.'

function workspaceView(requestId = 'request-1', body = originalBody): FeedbackWorkspaceView {
  return {
    request: {
      request_id: requestId, host_id: 'codex', host_session_id: 'session-1',
      source_hint: null, title: 'Review the work', what_happened: 'Need human feedback.',
      status: 'in_progress', resolution: null, allow_finish: false, final_summary: null,
      revision: 4, created_at: '2026-08-22T00:00:00Z', updated_at: '2026-08-22T00:00:00Z',
    },
    actions: [], context_refs: [], request_attachments: [], attachments: [], feedback: null,
    draft: {
      document_json: null, body_markdown: body, saved_revision: 4,
      updated_at: '2026-08-22T00:00:00Z',
    },
  }
}

function completedRequest(requestId = 'request-1'): FeedbackRequestView {
  return {
    request_id: requestId, host_id: 'codex', host_session_id: 'session-1',
    status: 'completed', execution_mode: 'wait', created_at: '2026-08-22T00:00:00Z',
    updated_at: '2026-08-22T00:01:00Z', feedback: { available: true },
    resolution: 'feedback_submitted', allow_finish: false, final_summary: null,
  }
}

function publishedFeedback() {
  return {
    manifest: {
      schema_version: 1, request_id: 'request-1', title: 'Review the work', host_id: 'codex',
      host_session_id: 'session-1', source_hint: null, submitted_at: '2026-08-22T00:01:00Z',
      source_revision: 4, draft_revision: 4, feedback_markdown: 'feedback.md',
      feedback_sha256: 'sha256', attachments: [],
    },
    markdown: 'Published feedback from the backend', uncooked_markdown: originalBody,
    attachment_paths: [],
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (cause: unknown) => void
  const promise = new Promise<T>((accept, decline) => { resolve = accept; reject = decline })
  return { promise, resolve, reject }
}

const cleanups: Array<() => void> = []
afterEach(() => { for (const cleanup of cleanups.splice(0)) cleanup() })

type PublisherContext = Parameters<typeof createPublisherController>[0]

/** Real UI sessions and save controller; only backend/device boundaries are controlled. */
function harness(options: Partial<PublisherContext> = {}) {
  const transport = new TestApplicationTransport()
    .handle('saveFeedbackDraft', (input) => ({
      document_json: input.document_json, body_markdown: input.body_markdown,
      saved_revision: input.expected_revision + 1, updated_at: '2026-08-22T00:00:30Z',
    }))
    .handle('submitFeedback', (input) => completedRequest(input.request_id))
    .resolve('readPublishedFeedback', publishedFeedback())
  const session = createWorkspaceSession()
  const draft = createDraftSession()
  const cooking = createCookingSession()
  const workspace = workspaceView()
  session.open(workspace)
  draft.adopt(workspace.draft)
  const drafts = createDraftController({
    transport, session: draft, messageFrom: (cause) => String(cause),
    isInteractionLocked: () => get(session).interactionLocked,
    isWorkspaceTerminal: () => get(session).terminal,
    getWorkspace: () => get(session).workspace,
    setWorkspaceDraft: session.setDraft,
  })
  cleanups.push(drafts.cancelPendingSave)
  const setPageError = vi.fn()
  const showSubmittedToast = vi.fn()
  const refreshNavigation = vi.fn(async () => {})
  const saveDraftNow = vi.fn(drafts.saveDraftNow)
  const cookSubmission = vi.fn<PublisherContext['cookSubmission']>(async (submission) => ({
    requestId: submission.request.request_id, savedRevision: submission.savedRevision,
    original: submission.body, markdown: 'Cooked feedback', model: 'test/model',
  }))
  const publisher = createPublisherController({
    transport, session, draft, cooking, setPageError, showSubmittedToast,
    refreshNavigation, saveDraftNow, cookSubmission,
    tr: (source) => source, messageFrom: String, isReadOnly: () => false,
    prepareFeedback: async () => ({ kind: 'ready' }),
    getCookingEnabled: () => false,
    ...options,
  })
  function edit(body: string) {
    drafts.updateDraft(snapshotFeedbackDraftDocument({
      type: 'doc', content: [{ type: 'paragraph', content: body ? [{ type: 'text', text: body }] : [] }],
    }))
  }
  function open(requestId: string, body: string) {
    session.close()
    const next = workspaceView(requestId, body)
    session.open(next)
    draft.adopt(next.draft)
  }
  return {
    transport, session, draft, cooking, drafts, publisher, edit, open,
    saveDraftNow, cookSubmission, setPageError, showSubmittedToast, refreshNavigation,
  }
}

describe('publisherController', () => {
  it('re-reads after an invalidation race without resubmitting or reporting failure', async () => {
    const h = harness()
    let reads = 0
    h.transport.handle('readPublishedFeedback', () => {
      if (++reads === 1) throw new StaleHttpApplicationResponseError()
      return publishedFeedback()
    })
    await h.publisher.submitFeedback()
    expect(h.transport.callsFor('submitFeedback')).toHaveLength(1)
    expect(h.transport.callsFor('readPublishedFeedback')).toHaveLength(2)
    expect(get(h.session).publishedFeedback?.markdown).toBe('Published feedback from the backend')
    expect(get(h.session).feedbackResult).toEqual({ available: true })
    expect(h.setPageError.mock.calls).toEqual([['']])
    expect(h.showSubmittedToast).toHaveBeenCalledOnce()
  })

  it.each(['', '  \n\t  '])('rejects empty replies before stopping speech or saving: %j', async (body) => {
    const prepareFeedback = vi.fn(async () => ({ kind: 'ready' as const }))
    const h = harness({ prepareFeedback })
    h.edit(body)
    await h.publisher.submitFeedback()
    expect(h.setPageError).toHaveBeenCalledWith('Cannot send an empty reply. Write some feedback content first.')
    expect(prepareFeedback).not.toHaveBeenCalled()
    expect(h.saveDraftNow).not.toHaveBeenCalled()
    expect(h.transport.callsFor('submitFeedback')).toEqual([])
  })

  it('checks pending speech after stopping recording, before saving or publishing', async () => {
    const h = harness({
      prepareFeedback: async () => ({ kind: 'pending-speech' }),
    })
    await h.publisher.submitFeedback()
    expect(h.setPageError).toHaveBeenCalledWith('Review the pending speech in the capsule before submitting feedback.')
    expect(h.saveDraftNow).not.toHaveBeenCalled()
    expect(h.transport.callsFor('submitFeedback')).toEqual([])
    expect(get(h.session).interactionLocked).toBe(false)
  })

  it('publishes an existing cooked preview without replacing the canonical draft or cooking again', async () => {
    const h = harness({ getCookingEnabled: () => true })
    const before = h.draft.snapshot()
    h.cooking.setPreview({
      requestId: 'request-1', savedRevision: 4, original: originalBody,
      markdown: 'Already reviewed cooked feedback', model: 'test/reviewed',
    })
    await h.publisher.submitFeedback()
    expect(h.cookSubmission).not.toHaveBeenCalled()
    expect(h.draft.snapshot()).toEqual(before)
    expect(h.transport.callsFor('submitFeedback')).toEqual([{
      name: 'submitFeedback', input: {
        request_id: 'request-1', expected_revision: 4,
        cooked_markdown: 'Already reviewed cooked feedback', cooking_model: 'test/reviewed',
        uncooked_markdown: originalBody,
      },
    }])
    expect(get(h.session).terminal).toBe(true)
    expect(h.cooking.preview()).toBeNull()
  })

  it('shares a double click across speech and saving, then publishes the final accepted revision once', async () => {
    const speech = deferred<void>()
    const saved = deferred<DraftView>()
    const h = harness({ prepareFeedback: async () => { await speech.promise; return { kind: 'ready' } } })
    h.transport.handle('saveFeedbackDraft', () => saved.promise)
    const first = h.publisher.submitFeedback()
    const second = h.publisher.submitFeedback()
    expect(second).toBe(first)
    await Promise.resolve()
    // Final speech may still enter the document before preparation finishes.
    h.edit('Feedback including the final speech segment.')
    const finalSnapshot = h.draft.snapshot()
    speech.resolve()
    await vi.waitFor(() => expect(h.transport.callsFor('saveFeedbackDraft')).toHaveLength(1))
    expect(get(h.session)).toMatchObject({ interactionLocked: true, submitStage: 'saving' })
    h.edit('A late edit must not replace the frozen submission.')
    expect(h.draft.snapshot()).toEqual(finalSnapshot)
    expect(h.publisher.submitFeedback()).toBe(first)
    saved.resolve({
      document_json: finalSnapshot.documentJson, body_markdown: finalSnapshot.bodyMarkdown,
      saved_revision: 5, updated_at: '2026-08-22T00:00:30Z',
    })
    await first
    expect(h.transport.callsFor('submitFeedback')).toEqual([{
      name: 'submitFeedback', input: { request_id: 'request-1', expected_revision: 5 },
    }])
    expect(get(h.draft)).toMatchObject({ dirty: false, savedRevision: 5 })
    expect(get(h.session)).toMatchObject({ terminal: true, interactionLocked: false, submitStage: 'idle' })
  })

  it('keeps failed saves editable and retries publication only after an explicit retry saves successfully', async () => {
    const h = harness()
    h.edit('Human edits that must survive a temporary save failure.')
    const unsaved = h.draft.snapshot()
    let saves = 0
    h.transport.handle('saveFeedbackDraft', (input) => {
      if (++saves === 1) throw new Error('Storage temporarily unavailable')
      return {
        document_json: input.document_json, body_markdown: input.body_markdown,
        saved_revision: 5, updated_at: '2026-08-22T00:00:30Z',
      }
    })
    await h.publisher.submitFeedback()
    expect(h.transport.callsFor('saveFeedbackDraft')).toHaveLength(1)
    expect(h.transport.callsFor('submitFeedback')).toEqual([])
    expect(h.draft.snapshot()).toEqual(unsaved)
    expect(get(h.draft)).toMatchObject({ dirty: true, phase: 'error', message: 'Error: Storage temporarily unavailable' })
    expect(get(h.session)).toMatchObject({ terminal: false, interactionLocked: false, submitStage: 'idle' })
    await h.publisher.submitFeedback()
    expect(h.transport.callsFor('saveFeedbackDraft')).toHaveLength(2)
    expect(h.transport.callsFor('submitFeedback')).toHaveLength(1)
    expect(get(h.draft)).toMatchObject({ dirty: false, savedRevision: 5 })
    expect(get(h.session).terminal).toBe(true)
  })

  it.each(['published read', 'navigation refresh'] as const)('retains committed feedback after a failed %s and never resubmits', async (failure) => {
    const refreshNavigation = vi.fn(async () => {
      if (failure === 'navigation refresh') throw new Error('Navigation unavailable')
    })
    const h = harness({ refreshNavigation })
    if (failure === 'published read') h.transport.reject('readPublishedFeedback', new Error('Published read unavailable'))
    await h.publisher.submitFeedback()
    await h.publisher.submitFeedback()
    expect(h.transport.callsFor('submitFeedback')).toHaveLength(1)
    expect(get(h.session)).toMatchObject({
      terminal: true, feedbackResult: { available: true }, interactionLocked: false,
      completedResult: { resolution: 'feedback_submitted' },
    })
    expect(h.setPageError).toHaveBeenLastCalledWith(
      failure === 'published read' ? 'Error: Published read unavailable' : 'Error: Navigation unavailable',
    )
    expect(h.showSubmittedToast).toHaveBeenCalledOnce()
    expect(refreshNavigation).toHaveBeenCalledOnce()
    if (failure === 'published read') expect(get(h.session).publishedFeedback).toBeNull()
  })

  it('preserves the source and releases both locks when Cooking fails, allowing a subsequent submission', async () => {
    const h = harness({ getCookingEnabled: () => true })
    const cookingResult = deferred<never>()
    h.cookSubmission.mockImplementationOnce(() => cookingResult.promise)
    const original = h.draft.snapshot()
    const submission = h.publisher.submitFeedback()
    await vi.waitFor(() => expect(h.cookSubmission).toHaveBeenCalledOnce())
    expect(get(h.session)).toMatchObject({ interactionLocked: true, submitStage: 'cooking' })
    expect(h.cooking.isCooking('request-1')).toBe(true)
    cookingResult.reject(new Error('Model unavailable'))
    await submission
    expect(h.draft.snapshot()).toEqual(original)
    expect(h.transport.callsFor('submitFeedback')).toEqual([])
    expect(h.setPageError).toHaveBeenLastCalledWith('Error: Model unavailable')
    expect(get(h.session)).toMatchObject({ terminal: false, interactionLocked: false, submitStage: 'idle' })
    expect(h.cooking.isCooking('request-1')).toBe(false)
    await h.publisher.submitFeedback()
    expect(h.transport.callsFor('submitFeedback')).toHaveLength(1)
    expect(h.draft.snapshot()).toEqual(original)
  })

  it.each([
    { requestId: 'another-request' },
    { savedRevision: 3 },
    { original: 'An earlier version of the human feedback.' },
  ])('rejects a preview whose source no longer matches: %j', async (mismatch) => {
    const h = harness({ getCookingEnabled: () => true })
    const preview = {
      requestId: 'request-1', savedRevision: 4, original: originalBody,
      markdown: 'Previously reviewed cooked text', model: 'test/model', ...mismatch,
    }
    h.cooking.setPreview(preview)
    await h.publisher.submitFeedback()
    expect(h.transport.callsFor('submitFeedback')).toEqual([])
    expect(h.cookSubmission).not.toHaveBeenCalled()
    expect(h.cooking.preview()).toEqual(preview)
    expect(h.setPageError).toHaveBeenLastCalledWith('The draft changed after Cooking. Restore the original and Cook again.')
    expect(get(h.session).interactionLocked).toBe(false)
  })

  it.each(['finishes', 'fails'] as const)('does not touch the next request when speech preparation %s after navigation', async (outcome) => {
    const speech = deferred<void>()
    const h = harness({ prepareFeedback: async () => { await speech.promise; return { kind: 'ready' } } })
    const submission = h.publisher.submitFeedback()
    await Promise.resolve()
    h.open('request-2', 'A different request and its own draft.')
    const nextDraft = h.draft.snapshot()
    if (outcome === 'finishes') speech.resolve()
    else speech.reject(new Error('Old microphone failure'))
    await submission
    expect(h.saveDraftNow).not.toHaveBeenCalled()
    expect(h.transport.callsFor('submitFeedback')).toEqual([])
    expect(h.draft.snapshot()).toEqual(nextDraft)
    expect(get(h.session)).toMatchObject({ request: { request_id: 'request-2' }, feedbackResult: null, interactionLocked: false })
    expect(h.setPageError).not.toHaveBeenCalled()
  })
})

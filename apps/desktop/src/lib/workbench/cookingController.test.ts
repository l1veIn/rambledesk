import { get } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { snapshotFeedbackDraftDocument } from '../feedbackDraftDocument'
import { PreviewApplicationTransport } from '../preview/previewApplicationTransport'
import { previewFixtures } from '../preview/previewFixtures'
import { UNAVAILABLE_CAPABILITY_MANIFEST } from '../capabilities/unavailableCapabilities'
import { createCookingController } from './cookingController'
import { createCookingSession } from './cookingSession'
import { createDraftController } from './draftController'
import { createDraftSession } from './draftSession'
import { createWorkspaceSession } from './workspaceSession'
import type { FeedbackPreparation } from '../speech/rambleSessionControllerHandle'

const model = vi.hoisted(() => ({ cookFeedback: vi.fn() }))
vi.mock('../cooking', () => model)
const cleanups: Array<() => void> = []
beforeEach(() => { model.cookFeedback.mockReset(); model.cookFeedback.mockResolvedValue({ markdown: 'Organized variant', model: 'test/model' }) })
afterEach(() => { for (const cleanup of cleanups.splice(0)) cleanup() })
function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => { resolve = done })
  return { promise, resolve }
}
function harness() {
  const transport = new PreviewApplicationTransport(UNAVAILABLE_CAPABILITY_MANIFEST)
  const session = createWorkspaceSession()
  const draft = createDraftSession()
  const cooking = createCookingSession()
  session.open(previewFixtures.workspace)
  draft.adopt(previewFixtures.workspace.draft)
  const drafts = createDraftController({
    transport, session: draft, messageFrom: String,
    isInteractionLocked: () => get(session).interactionLocked,
    isWorkspaceTerminal: () => get(session).terminal,
    getWorkspace: () => get(session).workspace, setWorkspaceDraft: session.setDraft,
  })
  cleanups.push(drafts.cancelPendingSave)
  let enabled = true
  const prepareFeedback = vi.fn(async (): Promise<FeedbackPreparation> => ({ kind: 'ready' }))
  const setPageError = vi.fn()
  const controller = createCookingController({
    tr: (source) => source, messageFrom: String, getWorkspace: () => get(session).workspace,
    getDraftBody: () => get(draft).body, getCookingConfig: () => ({
      provider: 'compatible', baseUrl: 'https://example.invalid/v1', apiKey: '',
      model: 'test', locale: 'en', systemPrompt: '', reasoningEffort: 'none',
    }),
    isCookingEnabled: () => enabled, isCooking: () => cooking.isCooking(session.requestId()),
    prepareFeedback, saveDraftNow: drafts.saveDraftNow, setPageError,
    setCooking: cooking.setCooking, setPreview: cooking.setPreview,
  })
  const edit = (body: string) => drafts.updateDraft(snapshotFeedbackDraftDocument({
    type: 'doc', content: [{ type: 'paragraph', content: [{ type: 'text', text: body }] }],
  }))
  return { transport, session, draft, cooking, controller, prepareFeedback, setPageError, edit, disable: () => { enabled = false } }
}

describe('Cooking preview at the real input and draft boundary', () => {
  it('shares preparation, saves once, and preserves the canonical draft beside its versioned variant', async () => {
    const h = harness()
    const preparation = deferred<FeedbackPreparation>()
    h.prepareFeedback.mockReturnValue(preparation.promise)
    const first = h.controller.cookPreviewOnly()
    expect(h.controller.cookPreviewOnly()).toBe(first)
    h.edit('Final speech included in the original')
    preparation.resolve({ kind: 'ready' })
    await first
    expect(model.cookFeedback).toHaveBeenCalledOnce()
    expect(get(h.draft)).toMatchObject({ body: 'Final speech included in the original', dirty: false })
    expect(get(h.cooking)).toMatchObject({ preview: {
      requestId: previewFixtures.workspace.request.request_id,
      savedRevision: get(h.draft).savedRevision,
      original: 'Final speech included in the original', markdown: 'Organized variant',
    } })
    h.controller.restoreOriginal()
    expect(get(h.cooking).preview).toBeNull()
    expect(get(h.draft).body).toBe('Final speech included in the original')
  })

  it.each<FeedbackPreparation>([{ kind: 'pending-speech' }, { kind: 'failed', message: 'Microphone could not stop' }])('does not bypass an unresolved input result: $kind', async (preparation) => {
    const h = harness()
    h.prepareFeedback.mockResolvedValue(preparation)
    await h.controller.cookPreviewOnly()
    expect(model.cookFeedback).not.toHaveBeenCalled()
    expect(get(h.cooking).preview).toBeNull()
    expect(h.setPageError).toHaveBeenCalled()
  })

  it('does not cook after a save failure and preserves the unsaved source', async () => {
    const h = harness()
    h.edit('Keep this source')
    const call = h.transport.call.bind(h.transport)
    h.transport.call = (async (name, input) => {
      if (name === 'saveFeedbackDraft') throw new Error('Disk unavailable')
      return call(name, input)
    }) as typeof h.transport.call
    await h.controller.cookPreviewOnly()
    expect(get(h.draft)).toMatchObject({ body: 'Keep this source', dirty: true })
    expect(model.cookFeedback).not.toHaveBeenCalled()
    expect(get(h.cooking).cookingRequestIds.size).toBe(0)
  })

  it.each(['disabled', 'changed'] as const)('drops a late variant when its source is %s', async (reason) => {
    const h = harness()
    const result = deferred<{ markdown: string; model: string }>()
    model.cookFeedback.mockReturnValue(result.promise)
    const flight = h.controller.cookPreviewOnly()
    await vi.waitFor(() => expect(model.cookFeedback).toHaveBeenCalledOnce())
    if (reason === 'disabled') h.disable()
    else h.edit('Newer human edits')
    result.resolve({ markdown: 'Old variant', model: 'test/model' })
    await flight
    expect(get(h.cooking).preview).toBeNull()
    expect(get(h.cooking).cookingRequestIds.size).toBe(0)
  })
})

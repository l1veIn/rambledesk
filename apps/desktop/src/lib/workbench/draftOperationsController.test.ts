import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({ readApplicationSnapshot: vi.fn() }))
vi.mock('../application/readApplicationSnapshot', () => ({
  readApplicationSnapshot: mocks.readApplicationSnapshot,
}))

import { previewFixtures } from '../preview/previewFixtures'
import { requestTaskViewDescriptor, sessionViewDescriptor } from '../workspace/viewDescriptors'
import type { FeedbackWorkspaceView } from '../feedback'
import type { FeedbackDraftSnapshot } from '../feedbackDraftDocument'
import type { SpeechWriteInput } from '../speech/speechWriteback'
import {
  createDraftOperationsController,
  type DraftOperationsContext,
} from './draftOperationsController'

const workspace = previewFixtures.workspace
const request = workspace.request

function harness(overrides: Partial<DraftOperationsContext> = {}) {
  const context = {
    transport: { call: vi.fn(async () => null) },
    tr: (source: string) => source,
    messageFrom: (cause: unknown) => String(cause),
    isPreviewMode: () => false,
    getActiveView: () => null,
    getPendingViewKey: () => null,
    getWorkspace: () => null,
    getCurrentRequest: () => null,
    getEditor: () => undefined,
    isWorkbenchMounted: () => false,
    isTransitionLocked: () => false,
    isInputLocked: () => false,
    getDraftMessage: () => '',
    getDraftSnapshot: () => ({ documentJson: workspace.draft.document_json ?? '', bodyMarkdown: workspace.draft.body_markdown }),
    updateDraft: vi.fn(),
    saveDraftNow: vi.fn(async () => true),
    setWorkspaceDraft: vi.fn(),
    adoptDraft: vi.fn(),
    setPageError: vi.fn(),
    ...overrides,
  } as unknown as DraftOperationsContext
  return { controller: createDraftOperationsController(context), context }
}

describe('draft operations controller', () => {
  it('applies a foreground operation through the mounted editor and saves', async () => {
    const applyDraftOperation = vi.fn(() => true)
    const saveDraftNow = vi.fn(async () => true)
    const { controller, context } = harness({
      getActiveView: () => sessionViewDescriptor(request.host_id, request.host_session_id),
      isWorkbenchMounted: () => true,
      getWorkspace: () => workspace,
      getEditor: () => ({ applyDraftOperation } as never),
      saveDraftNow,
    })

    await controller.routeDraftOperation(request.request_id, {
      kind: 'clearActionGroup',
      actionId: 'a',
    })

    expect(applyDraftOperation).toHaveBeenCalledWith({ kind: 'clearActionGroup', actionId: 'a' })
    expect(saveDraftNow).toHaveBeenCalled()
    expect(context.setPageError).not.toHaveBeenCalled()
  })

  it('refuses to write through the editor of a closed request', async () => {
    const { controller, context } = harness({
      getActiveView: () => sessionViewDescriptor(request.host_id, request.host_session_id),
      isWorkbenchMounted: () => true,
      getWorkspace: () => ({
        ...workspace,
        request: { ...request, status: 'completed' },
      }),
      getEditor: () => ({ applyDraftOperation: vi.fn(() => true) } as never),
    })

    await expect(
      controller.routeDraftOperation(request.request_id, { kind: 'clearActionGroup', actionId: 'a' }),
    ).rejects.toThrow('read-only')
    expect(context.setPageError).toHaveBeenCalled()
  })

  it('writes a background operation and adopts the saved draft for an active task view', async () => {
    mocks.readApplicationSnapshot.mockResolvedValue(workspace)
    const call = vi.fn(async (_name: string, input: Record<string, unknown>) => ({
      document_json: input.document_json,
      body_markdown: input.body_markdown,
      saved_revision: Number(input.expected_revision) + 1,
      updated_at: null,
    }))
    const { controller, context } = harness({
      transport: { call } as never,
      getActiveView: () => requestTaskViewDescriptor(request.request_id),
      getCurrentRequest: () => request,
      getWorkspace: () => workspace,
    })

    await controller.routeDraftOperation(request.request_id, {
      kind: 'appendClipboardText',
      text: 'hello',
      label: 'Clipboard',
      action: null,
    })

    expect(call).toHaveBeenCalledWith(
      'saveFeedbackDraft',
      expect.objectContaining({ request_id: request.request_id }),
    )
    expect(context.setWorkspaceDraft).toHaveBeenCalled()
    expect(context.adoptDraft).toHaveBeenCalled()
  })

  it('reports a failed background write and rethrows', async () => {
    mocks.readApplicationSnapshot.mockRejectedValue(new Error('offline'))
    const { controller, context } = harness()

    await expect(
      controller.routeDraftOperation(request.request_id, { kind: 'clearActionGroup', actionId: 'a' }),
    ).rejects.toThrow('offline')
    expect(context.setPageError).toHaveBeenCalledWith('Failed to write Ramble content: {error}')
  })

  it('serializes queued document tasks', async () => {
    const order: string[] = []
    const { controller } = harness()
    const first = controller.enqueueDocumentTask(async () => {
      await Promise.resolve()
      order.push('first')
    })
    const second = controller.enqueueDocumentTask(async () => {
      order.push('second')
    })
    await Promise.all([first, second])
    expect(order).toEqual(['first', 'second'])
  })

  it('toggles the active action group for the open request', () => {
    const { controller } = harness({ getCurrentRequest: () => request })
    controller.selectAction('action-1', 0, 'Action one')
    expect(controller.activeActionId(request.request_id)).toBe('action-1')
    expect(controller.activeActionFor(request.request_id)).toEqual({
      actionId: 'action-1',
      actionIndex: 0,
      title: 'Action one',
    })
    controller.selectAction('action-1', 0, 'Action one')
    expect(controller.activeActionId(request.request_id)).toBeNull()
  })

  it('ignores a selection while a view transition is locked', () => {
    const { controller } = harness({
      getCurrentRequest: () => request,
      isTransitionLocked: () => true,
    })
    controller.selectAction('action-1', 0, 'Action one')
    expect(controller.activeActionFor(request.request_id)).toBeNull()
  })
})

function reviewSnapshot(body: string): FeedbackDraftSnapshot {
  return {
    documentJson: JSON.stringify({
      schemaVersion: 2,
      doc: { type: 'doc', content: [{ type: 'paragraph', content: [{ type: 'text', text: 'Unsubmitted general notes' }] }] },
      workbenchState: { type: 'document_review', verdict: null, paragraph_marks: [], annotations: [{
        id: 'annotation-1', paragraph_id: 'paragraph-1', start: null, end: null, quote: null,
        kind: 'comment', body, replacement: null,
      }] },
    }),
    bodyMarkdown: 'Unsubmitted general notes',
  }
}

function reviewWorkspace(body = 'Saved comment', revision = 1): FeedbackWorkspaceView {
  const snapshot = reviewSnapshot(body)
  return {
    ...workspace,
    request: { ...request, request_id: 'review-1', status: 'in_progress' },
    workbench: { type: 'document_review', version: 1, data: {
      title: 'Script', source_version: 'source-v1', paragraphs: [{ id: 'paragraph-1', text: 'Immutable source text' }],
    } },
    draft: { ...workspace.draft, document_json: snapshot.documentJson, body_markdown: snapshot.bodyMarkdown, saved_revision: revision },
  }
}

const annotationSpeech: SpeechWriteInput = {
  requestId: 'review-1', requestTitle: 'Script', id: 'speech-annotation-1', text: 'Spoken comment',
  destination: { kind: 'review_annotation', annotationId: 'annotation-1', field: 'body', sourceVersion: 'source-v1', paragraphLabel: 'First paragraph' },
}

describe('annotation speech routing', () => {
  beforeEach(() => mocks.readApplicationSnapshot.mockReset())

  it('pins manual input to the latest unsaved annotation without speech provenance', async () => {
    const saved = reviewWorkspace()
    let local = reviewSnapshot('Unsaved typing')
    const { controller } = harness({ getWorkspace: () => saved, getDraftSnapshot: () => local,
      updateDraft: (next) => { local = next } })
    const target = structuredClone(annotationSpeech)
    const write = controller.routeInputText(target, '![image](attachment://1)', 'input-1')
    target.destination = { kind: 'document', action: null }
    await write
    const result = JSON.parse(local.documentJson)
    expect(result.workbenchState.annotations[0].body).toBe('Unsaved typing\n![image](attachment://1)')
    expect(local.bodyMarkdown).toBe('Unsubmitted general notes')
    expect(result.speechWriteback).toBeUndefined()
    expect(result.fieldSpeechSegments).toBeUndefined()
    expect(result.inputWriteback.operations).toHaveLength(1)
  })

  it('appends to the unsaved local annotation and keeps unrelated notes', async () => {
    const saved = reviewWorkspace()
    let local = reviewSnapshot('Unsaved typing')
    const updateDraft = vi.fn((next: FeedbackDraftSnapshot) => { local = next })
    const { controller, context } = harness({
      getWorkspace: () => saved,
      getCurrentRequest: () => saved.request,
      getDraftSnapshot: () => local,
      updateDraft,
    })

    await controller.routeSpeech(annotationSpeech)

    expect(JSON.parse(local.documentJson).workbenchState.annotations[0].body).toBe('Unsaved typing\nSpoken comment')
    expect(local.bodyMarkdown).toBe('Unsubmitted general notes')
    expect(JSON.parse(local.documentJson).doc).toEqual(JSON.parse(reviewSnapshot('').documentJson).doc)
    expect(context.saveDraftNow).toHaveBeenCalledOnce()
    expect(mocks.readApplicationSnapshot).not.toHaveBeenCalled()
    expect(context.transport.call).not.toHaveBeenCalled()
  })

  it('retries a failed local save without appending the same speech twice', async () => {
    const saved = reviewWorkspace()
    let local = reviewSnapshot('Draft comment')
    const saveDraftNow = vi.fn().mockResolvedValueOnce(false).mockResolvedValueOnce(true)
    const { controller } = harness({
      getWorkspace: () => saved,
      getDraftSnapshot: () => local,
      updateDraft: (next) => { local = next },
      saveDraftNow,
      getDraftMessage: () => 'Save unavailable',
    })

    await expect(controller.routeSpeech(annotationSpeech)).rejects.toThrow('Save unavailable')
    const firstWrite = local.documentJson
    await controller.routeSpeech(annotationSpeech)
    expect(local.documentJson).toBe(firstWrite)
    expect(JSON.parse(local.documentJson).workbenchState.annotations[0].body).toBe('Draft comment\nSpoken comment')
    expect(JSON.parse(local.documentJson).speechWriteback.operations).toHaveLength(1)
    expect(saveDraftNow).toHaveBeenCalledTimes(2)
  })

  it('uses the background request CAS and preserves concurrent annotation edits after conflict', async () => {
    mocks.readApplicationSnapshot
      .mockResolvedValueOnce(reviewWorkspace('Original comment', 3))
      .mockResolvedValueOnce(reviewWorkspace('Concurrent edit', 4))
    const call = vi.fn()
      .mockRejectedValueOnce({ code: 'DRAFT_CONFLICT', message: 'stale revision' })
      .mockImplementationOnce(async (_command, input) => ({
        document_json: input.document_json, body_markdown: input.body_markdown, saved_revision: 5, updated_at: '',
      }))
    const { controller, context } = harness({
      transport: { call } as never,
      getWorkspace: () => workspace,
      getCurrentRequest: () => workspace.request,
    })

    await controller.routeSpeech(annotationSpeech)

    expect(mocks.readApplicationSnapshot).toHaveBeenCalledTimes(2)
    expect(call.mock.calls.map(([, input]) => input.expected_revision)).toEqual([3, 4])
    expect(call.mock.calls.every(([command, input]) => command === 'saveFeedbackDraft' && input.request_id === 'review-1')).toBe(true)
    const persisted = JSON.parse(call.mock.calls[1][1].document_json)
    expect(persisted.workbenchState.annotations[0].body).toBe('Concurrent edit\nSpoken comment')
    expect(persisted.speechWriteback.operations).toHaveLength(1)
    expect(context.updateDraft).not.toHaveBeenCalled()
    expect(context.adoptDraft).not.toHaveBeenCalled()
  })

  it('rejects an annotation write while the visible request input is frozen', async () => {
    const saved = reviewWorkspace()
    const { controller, context } = harness({
      getWorkspace: () => saved,
      isInputLocked: () => true,
      getDraftSnapshot: () => reviewSnapshot('Do not change'),
    })
    await expect(controller.routeSpeech(annotationSpeech)).rejects.toThrow()
    expect(context.updateDraft).not.toHaveBeenCalled()
    expect(context.saveDraftNow).not.toHaveBeenCalled()
    expect(mocks.readApplicationSnapshot).not.toHaveBeenCalled()
  })
})

describe('document speech support guard', () => {
  beforeEach(() => mocks.readApplicationSnapshot.mockReset())

  it.each(['foreground', 'background'] as const)('rejects an unknown workbench version before a %s write', async (route) => {
    const unsupported: FeedbackWorkspaceView = {
      ...workspace,
      workbench: { type: 'ramble', version: 99, data: { actions: [{ id: 'review', instruction: 'Review this result' }] } },
    }
    const originalDraft = structuredClone(unsupported.draft)
    const applyDraftOperation = vi.fn(() => true)
    mocks.readApplicationSnapshot.mockResolvedValue(unsupported)
    const { controller, context } = harness({
      tr: (source, values) => source.replace('{error}', String(values?.error ?? '')),
      getWorkspace: () => route === 'foreground' ? unsupported : null,
      getActiveView: () => route === 'foreground' ? sessionViewDescriptor(request.host_id, request.host_session_id) : null,
      isWorkbenchMounted: () => route === 'foreground',
      getEditor: () => ({ applyDraftOperation } as never),
    })
    const speech: SpeechWriteInput = {
      requestId: request.request_id, requestTitle: request.title,
      id: 'preserved-speech', text: 'Keep these words pending', destination: { kind: 'document', action: null },
    }

    await expect(controller.routeSpeech(speech)).rejects.toThrow('This workbench is not supported. The draft is read-only.')

    expect(applyDraftOperation).not.toHaveBeenCalled()
    expect(context.saveDraftNow).not.toHaveBeenCalled()
    expect(context.transport.call).not.toHaveBeenCalled()
    expect(context.adoptDraft).not.toHaveBeenCalled()
    expect(unsupported.draft).toEqual(originalDraft)
    expect(mocks.readApplicationSnapshot).toHaveBeenCalledTimes(route === 'foreground' ? 0 : 1)
    expect(context.setPageError).toHaveBeenCalledWith('Failed to write Ramble content: Error: This workbench is not supported. The draft is read-only.')
  })
})

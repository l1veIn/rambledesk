import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { FeedbackWorkspaceView } from '../feedback'
import type { SpeechTarget } from '../speech/speechTargets'
import { snapshotFeedbackDraftDocument } from '../feedbackDraftDocument'
import { createAttachmentController } from './attachmentController'
import { controllerContext, fileCandidate, mocks, resetAttachmentMocks, screenCandidate } from './attachmentControllerTestHarness'

const target: SpeechTarget = { requestId: 'r', requestTitle: 'Review', destination: {
  kind: 'review_annotation', annotationId: 'comment', field: 'body', sourceVersion: 'v1', paragraphLabel: 'Opening',
} }
function workspace(): FeedbackWorkspaceView {
  const snapshot = snapshotFeedbackDraftDocument({ type: 'doc', content: [] })
  return { request: { request_id: 'r', status: 'in_progress' }, attachments: [],
    workbench: { type: 'document_review', version: 1, data: {
      title: 'Script', source_version: 'v1', paragraphs: [{ id: 'p', text: 'Manuscript' }],
    } }, draft: { saved_revision: 1, body_markdown: '', document_json: JSON.stringify({ ...JSON.parse(snapshot.documentJson),
      workbenchState: { type: 'document_review', verdict: null, paragraph_marks: [], annotations: [{ id: 'comment',
        paragraph_id: 'p', kind: 'comment', body: '', replacement: null, start: null, end: null, quote: null }] },
    }) },
  } as unknown as FeedbackWorkspaceView
}
function setup() {
  const { context, session } = controllerContext()
  const original = workspace()
  let current = original
  context.getWorkspace = () => current
  context.routeInputText = vi.fn(async () => {})
  const selected = structuredClone(target)
  context.getInputTarget = () => selected
  mocks.applicationCall.mockImplementation(async (command) => {
    if (command === 'getFeedbackWorkspace') return original
    if (command === 'addFeedbackAttachment') return { ...original, attachments: [{
      attachment_id: 'a1', file_name: 'reference.txt', media_type: 'text/plain',
    }] }
    return new ArrayBuffer(0)
  })
  return { context, session, selected, original, switchWorkspace: () => { current = { ...workspace(), request: { ...original.request, request_id: 'other' } } },
    controller: createAttachmentController(context) }
}

describe('field attachment acquisition', () => {
  beforeEach(resetAttachmentMocks)
  afterEach(() => vi.unstubAllGlobals())

  it('keeps the field selected when capture opened across target mutation and workspace navigation', async () => {
    const { context, selected, controller, switchWorkspace } = setup()
    const cleanup = controller.mount()
    await controller.startScreenCapture()
    selected.destination = { kind: 'document', action: null }
    switchWorkspace()
    mocks.listeners.get('screen-capture-ready')?.(screenCandidate())
    await vi.waitFor(() => expect(context.routeInputText).toHaveBeenCalledOnce())
    expect(context.routeInputText).toHaveBeenCalledWith(target, '[reference.txt](attachment://a1)', expect.any(String))
    expect(context.routeDraftOperation).not.toHaveBeenCalled()
    expect(await controller.prepareFeedback('r')).toEqual({ kind: 'ready' })
    cleanup()
  })

  it('uses an explicit picker target even after selection moved before the change event', async () => {
    const { context, selected, controller } = setup()
    selected.destination = { kind: 'document', action: null }
    const file = { name: 'reference.txt', size: 1, type: 'text/plain', arrayBuffer: async () => new Uint8Array([1]).buffer } as File
    expect(controller.handleFiles([file], target)).toBe(true)
    await vi.waitFor(() => expect(context.routeInputText).toHaveBeenCalledOnce())
    expect(context.routeInputText).toHaveBeenCalledWith(target, '[reference.txt](attachment://a1)', expect.any(String))
    expect(context.routeDraftOperation).not.toHaveBeenCalled()
  })

  it('waits for field reference writeback before allowing submission', async () => {
    const { context, controller } = setup()
    let finish!: () => void
    context.routeInputText = vi.fn(() => new Promise<void>((resolve) => { finish = resolve }))
    const importing = controller.importAttachmentCandidates([fileCandidate({ fileName: 'reference.txt', byteLength: 1,
      readBytes: async () => new Uint8Array([1]).buffer })])
    await vi.waitFor(() => expect(context.routeInputText).toHaveBeenCalledOnce())
    let ready = false
    const preparation = controller.prepareFeedback('r').then((value) => { ready = true; return value })
    await Promise.resolve()
    expect(ready).toBe(false)
    finish()
    await importing
    expect(await preparation).toEqual({ kind: 'ready' })
  })

  it('rejects deleted comments before storing bytes instead of falling back to the notes', async () => {
    const { context, controller, original } = setup()
    const envelope = JSON.parse(original.draft.document_json!)
    envelope.workbenchState.annotations = []
    original.draft.document_json = JSON.stringify(envelope)
    await controller.importAttachmentCandidates([fileCandidate({ fileName: 'reference.txt', byteLength: 1,
      readBytes: async () => new Uint8Array([1]).buffer })])
    expect(mocks.applicationCall.mock.calls.some(([command]) => command === 'addFeedbackAttachment')).toBe(false)
    expect(context.routeInputText).not.toHaveBeenCalled()
    expect(context.routeDraftOperation).not.toHaveBeenCalled()
    expect(await controller.prepareFeedback('r')).toMatchObject({ kind: 'failed' })
  })
})

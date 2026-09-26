import { describe, expect, it } from 'vitest'
import type { ReviewAnnotation } from '../generated/feedback'
import { decodeFeedbackDraftEnvelope, snapshotFeedbackDraftDocument } from '../feedbackDraftDocument'
import { applySpeechWriteback, speechDocumentOperation, type SpeechWriteInput, type SpeechWriteWorkspace } from './speechWriteback'

const annotation: ReviewAnnotation = { id: 'a', paragraph_id: 'p', start: 1, end: 2, quote: '😀', kind: 'suggestion', body: 'Existing note', replacement: '' }
const input: SpeechWriteInput = { requestId: 'r', requestTitle: 'Review', id: 'segment-1', text: 'Spoken words', destination: {
  kind: 'review_annotation', annotationId: 'a', field: 'body', sourceVersion: 'v1', paragraphLabel: 'Opening',
} }
function fixture(): SpeechWriteWorkspace {
  const snapshot = snapshotFeedbackDraftDocument({ type: 'doc', content: [{ type: 'paragraph', content: [{ type: 'text', text: 'Supplementary note' }] }] })
  return { request: { request_id: 'r', status: 'in_progress' }, workbench: { type: 'document_review', version: 1,
    data: { title: 'Script', source_version: 'v1', paragraphs: [{ id: 'p', text: 'A😀B' }] } },
  draft: { document_json: JSON.stringify({ ...JSON.parse(snapshot.documentJson), opaque: { nested: ['keep'] },
    workbenchState: { type: 'document_review', verdict: null, annotations: [structuredClone(annotation)], paragraph_marks: [] } }), body_markdown: snapshot.bodyMarkdown } }
}
function updateAnnotation(workspace: SpeechWriteWorkspace, patch: Partial<ReviewAnnotation>): void {
  const envelope = JSON.parse(workspace.draft.document_json!)
  Object.assign(envelope.workbenchState.annotations[0], patch)
  workspace.draft.document_json = JSON.stringify(envelope)
}
function state(documentJson: string) { return JSON.parse(documentJson).workbenchState }

describe('speech writeback', () => {
  it('appends to a comment without changing source, anchors, decision, document, or opaque envelope fields', () => {
    const workspace = fixture()
    const original = structuredClone(workspace)
    const next = applySpeechWriteback(workspace, input)
    const before = decodeFeedbackDraftEnvelope(workspace.draft.document_json)!
    const after = decodeFeedbackDraftEnvelope(next.documentJson)!
    expect(state(next.documentJson).annotations[0]).toEqual({ ...annotation, body: 'Existing note\nSpoken words' })
    expect(state(next.documentJson).verdict).toBeNull()
    expect(after.doc).toEqual(before.doc)
    expect(after.opaque).toEqual(before.opaque)
    expect(next.bodyMarkdown).toBe(workspace.draft.body_markdown)
    expect(workspace).toEqual(original)
    expect(after.speechWriteback).toMatchObject({ version: 1, operations: [{ id: input.id, annotationId: 'a', field: 'body' }] })
  })

  it('appends suggested wording separately and leaves deliberate deletion empty until speech is written', () => {
    const workspace = fixture()
    const replacement = { ...input, destination: { ...input.destination, field: 'replacement' } } as SpeechWriteInput
    expect(state(applySpeechWriteback(workspace, replacement).documentJson).annotations[0])
      .toEqual({ ...annotation, replacement: input.text })
    updateAnnotation(workspace, { replacement: 'Existing wording\n' })
    expect(state(applySpeechWriteback(workspace, replacement).documentJson).annotations[0].replacement).toBe('Existing wording\nSpoken words')
  })

  it.each(['open', 'resolved'])('continues a legacy %s note without carrying its obsolete status forward', (status) => {
    const workspace = fixture()
    const envelope = JSON.parse(workspace.draft.document_json!)
    envelope.workbenchState.annotations[0].status = status
    workspace.draft.document_json = JSON.stringify(envelope)
    const next = applySpeechWriteback(workspace, input)
    expect(state(next.documentJson).annotations[0]).toEqual({ ...annotation, body: 'Existing note\nSpoken words' })
    expect(state(workspace.draft.document_json!).annotations[0].status).toBe(status)
  })

  it('uses durable receipts after reload and preserves later manual edits on an exact retry', () => {
    const workspace = fixture()
    const batch = { ...input, mergedIds: ['segment-1', 'segment-2'] }
    const first = applySpeechWriteback(workspace, batch)
    workspace.draft = { document_json: first.documentJson, body_markdown: first.bodyMarkdown }
    updateAnnotation(workspace, { body: 'Manually revised after acknowledgement was lost' })
    expect(applySpeechWriteback(workspace, batch)).toEqual({ documentJson: workspace.draft.document_json, bodyMarkdown: workspace.draft.body_markdown })
    expect(() => applySpeechWriteback(workspace, { ...batch, text: 'Different text' })).toThrow('different write')
    expect(() => applySpeechWriteback(workspace, { ...input, id: 'segment-2' })).toThrow('different write')
  })

  it.each(['request', 'closed', 'unknown', 'version', 'source', 'missing', 'duplicate', 'anchor', 'kind', 'history'] as const)
    ('retains caller text and rejects an invalid %s target', (problem) => {
      const workspace = fixture()
      const speech = structuredClone(input)
      if (problem === 'request') workspace.request.request_id = 'another'
      if (problem === 'closed') workspace.request.status = 'completed'
      if (problem === 'unknown') speech.destination = { kind: 'unknown', raw: { kind: 'future' } }
      if (problem === 'version') workspace.workbench!.version = 2
      if (problem === 'source' && speech.destination.kind === 'review_annotation') speech.destination.sourceVersion = 'v0'
      if (problem === 'missing') updateAnnotation(workspace, { id: 'gone' })
      if (problem === 'anchor') updateAnnotation(workspace, { quote: 'wrong' })
      if (problem === 'kind') {
        updateAnnotation(workspace, { kind: 'comment', replacement: null })
        if (speech.destination.kind === 'review_annotation') speech.destination.field = 'replacement'
      }
      if (problem === 'duplicate' || problem === 'history') {
        const envelope = JSON.parse(workspace.draft.document_json!)
        if (problem === 'duplicate') envelope.workbenchState.annotations.push({ ...annotation })
        else envelope.speechWriteback = { version: 2, operations: [] }
        workspace.draft.document_json = JSON.stringify(envelope)
      }
      const before = structuredClone(workspace)
      expect(() => applySpeechWriteback(workspace, speech)).toThrow()
      expect(workspace).toEqual(before)
      expect(speech.text).toBe(input.text)
    })

  it('counts Unicode scalars and refuses overflow without truncating speech', () => {
    const workspace = fixture()
    updateAnnotation(workspace, { body: '😀'.repeat(3998) })
    const next = applySpeechWriteback(workspace, { ...input, text: '文' })
    expect([...state(next.documentJson).annotations[0].body]).toHaveLength(4000)
    expect(() => applySpeechWriteback(workspace, { ...input, text: '文字' })).toThrow('text limit')
    updateAnnotation(workspace, { replacement: '😀'.repeat(7999) })
    expect(() => applySpeechWriteback(workspace, { ...input, destination: { ...input.destination, field: 'replacement' } } as SpeechWriteInput)).toThrow('text limit')
  })

  it('keeps the document operation path and its existing segment deduplication', () => {
    const workspace = fixture()
    const speech: SpeechWriteInput = { ...input, cleanupState: 'cleaned', destination: { kind: 'document', action: null } }
    expect(speechDocumentOperation(speech)).toEqual({ kind: 'appendSpeech', segmentId: input.id, text: input.text, action: null, cleanupState: 'cleaned' })
    const first = applySpeechWriteback(workspace, speech)
    workspace.draft = { document_json: first.documentJson, body_markdown: first.bodyMarkdown }
    expect(applySpeechWriteback(workspace, speech)).toEqual(first)
    expect(state(first.documentJson).annotations[0]).toEqual(annotation)
  })
})

import { describe, expect, it, vi } from 'vitest'
import type { FeedbackWorkspaceView, SaveDraftInput } from './feedback'
import { snapshotFeedbackDraftDocument } from './feedbackDraftDocument'
import { writeBackgroundInputText } from './backgroundDraftWriter'
import { applySpeechWriteback } from './speech/speechWriteback'
import { collectFieldSpeechSegments } from './speech/fieldSpeechSegments'
import type { SpeechTarget } from './speech/speechTargets'
import { applyInputTextWriteback, type InputTextWriteInput } from './inputTextWriteback'

const target: SpeechTarget = { requestId: 'r', requestTitle: 'Review', destination: {
  kind: 'review_annotation', annotationId: 'a', field: 'body', sourceVersion: 'v1', paragraphLabel: 'Opening',
} }
const input: InputTextWriteInput = { target, id: 'attachment-1', text: '![Reference](attachment://a1)' }

function fixture(kind: 'document_review' | 'questions' = 'document_review', text = ''): FeedbackWorkspaceView {
  const snapshot = snapshotFeedbackDraftDocument({ type: 'doc', content: [{ type: 'paragraph', content: [{ type: 'text', text: 'Unrelated notes' }] }] })
  const review = kind === 'document_review'
  return { request: { request_id: 'r', status: 'in_progress' },
    workbench: { type: kind, version: 1, data: review
      ? { title: 'Script', source_version: 'v1', paragraphs: [{ id: 'p', text: 'Original manuscript' }] }
      : { questions: [{ id: 'q', prompt: 'Choose', allowOther: true, options: [{ value: 'a', label: 'A' }, { value: 'b', label: 'B' }] }] } },
    draft: { document_json: JSON.stringify({ ...JSON.parse(snapshot.documentJson), opaque: { keep: true }, workbenchState: review
      ? { type: kind, verdict: null, paragraph_marks: [], annotations: [{ id: 'a', paragraph_id: 'p', start: null, end: null,
        quote: null, kind: 'suggestion', body: text, replacement: '' }] }
      : { type: kind, answers: [{ id: 'q', value: text, label: text, wasCustom: true }] } }),
      body_markdown: snapshot.bodyMarkdown, saved_revision: 1 }, attachments: [],
  } as unknown as FeedbackWorkspaceView
}

describe('field input text writeback', () => {
  it.each(['document_review', 'questions'] as const)('attaches to an empty %s field without changing notes or marking speech', (kind) => {
    const workspace = fixture(kind)
    const write = kind === 'document_review' ? input : { ...input, target: { ...target,
      destination: { kind: 'question_answer' as const, questionId: 'q', questionLabel: 'Question' } } }
    const next = applyInputTextWriteback(workspace, write)
    const envelope = JSON.parse(next.documentJson)
    const before = JSON.parse(workspace.draft.document_json!)
    expect(envelope.opaque).toEqual(before.opaque)
    expect(envelope.doc).toEqual(before.doc)
    expect(next.bodyMarkdown).toBe('Unrelated notes')
    expect(envelope.speechWriteback).toBeUndefined()
    expect(envelope.fieldSpeechSegments).toBeUndefined()
    if (kind === 'questions') expect(envelope.workbenchState.answers[0]).toMatchObject({ value: input.text, label: input.text })
    else expect(envelope.workbenchState.annotations[0]).toMatchObject({ body: input.text, replacement: '' })
  })

  it('deduplicates retried references without replacing subsequent manual edits', () => {
    const workspace = fixture()
    const first = applyInputTextWriteback(workspace, input)
    const envelope = JSON.parse(first.documentJson)
    envelope.workbenchState.annotations[0].body += '\nA later edit'
    workspace.draft.document_json = JSON.stringify(envelope)
    expect(applyInputTextWriteback(workspace, input).documentJson).toBe(workspace.draft.document_json)
    expect(() => applyInputTextWriteback(workspace, { ...input, text: 'Different reference' })).toThrow('different write')
  })

  it('retains existing speech provenance while treating appended references as ordinary input', () => {
    const workspace = fixture()
    const spoken = applySpeechWriteback(workspace, { ...target, id: 'speech', text: 'Voice note' })
    workspace.draft.document_json = spoken.documentJson
    const next = applyInputTextWriteback(workspace, input)
    workspace.draft.document_json = next.documentJson
    expect(collectFieldSpeechSegments(workspace).map((segment) => segment.text)).toEqual(['Voice note'])
    expect(JSON.parse(next.documentJson).workbenchState.annotations[0].body).toBe(`Voice note\n${input.text}`)
  })

  it.each(['deleted', 'version', 'limit', 'closed', 'history'] as const)('rejects %s fields without writing to the document', (problem) => {
    const workspace = fixture('document_review', problem === 'limit' ? '😀'.repeat(4000) : '')
    const envelope = JSON.parse(workspace.draft.document_json!)
    if (problem === 'deleted') envelope.workbenchState.annotations = []
    if (problem === 'history') envelope.inputWriteback = { version: 2, operations: [] }
    if (problem === 'version') (workspace.workbench!.data as { source_version: string }).source_version = 'v2'
    if (problem === 'closed') workspace.request.status = 'completed'
    workspace.draft.document_json = JSON.stringify(envelope)
    const before = structuredClone(workspace)
    expect(() => applyInputTextWriteback(workspace, input)).toThrow()
    expect(workspace).toEqual(before)
  })

  it('rebases on concurrent typing after a background CAS conflict', async () => {
    let current = fixture('document_review', 'Initial')
    const save = vi.fn(async (saved: SaveDraftInput) => {
      if (saved.expected_revision === 1) {
        current = fixture('document_review', 'Concurrent typing')
        current.draft.saved_revision = 2
        throw { code: 'DRAFT_CONFLICT' }
      }
      return { document_json: saved.document_json, body_markdown: saved.body_markdown, saved_revision: 3, updated_at: '' }
    })
    const saved = await writeBackgroundInputText(input, { load: async () => current, save })
    expect(save).toHaveBeenCalledTimes(2)
    expect(JSON.parse(saved.document_json!).workbenchState.annotations[0].body).toBe(`Concurrent typing\n${input.text}`)
  })
})

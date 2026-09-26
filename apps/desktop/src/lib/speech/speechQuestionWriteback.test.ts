import { describe, expect, it, vi } from 'vitest'
import type { DraftView, FeedbackWorkspaceView, SaveDraftInput } from '../feedback'
import type { QuestionAnswer, QuestionsData } from '../generated/feedback'
import { snapshotFeedbackDraftDocument } from '../feedbackDraftDocument'
import { writeBackgroundSpeech } from '../backgroundDraftWriter'
import { applySpeechWriteback, type SpeechWriteInput } from './speechWriteback'

const input: SpeechWriteInput = { requestId: 'r', requestTitle: 'Questions', id: 'question-speech-1', text: 'Spoken answer',
  destination: { kind: 'question_answer', questionId: 'q1', questionLabel: 'Question one' } }

function fixture(value = 'Typed answer'): FeedbackWorkspaceView {
  const snapshot = snapshotFeedbackDraftDocument({ type: 'doc', content: [{ type: 'paragraph', content: [{ type: 'text', text: 'Notes stay separate' }] }] })
  const data: QuestionsData = { questions: ['q1', 'q2'].map((id) => ({ id, prompt: `Prompt ${id}`, allowOther: true,
    options: [{ value: 'yes', label: 'Yes' }, { value: 'no', label: 'No' }] })) }
  return {
    request: { request_id: 'r', host_id: 'host', host_session_id: 'session', source_hint: null, title: 'Questions', what_happened: '',
      status: 'in_progress', resolution: null, allow_finish: false, final_summary: null, revision: 1, created_at: '', updated_at: '' },
    workbench: { type: 'questions', version: 1, data }, actions: [], context_refs: [], request_attachments: [], attachments: [], feedback: null,
    draft: { document_json: JSON.stringify({ ...JSON.parse(snapshot.documentJson), opaque: { keep: ['nested'] }, workbenchState: { type: 'questions', answers: [
      { id: 'q1', value, label: value, wasCustom: true }, { id: 'q2', value: 'yes', label: 'Yes', wasCustom: false, index: 1 },
    ] } }), body_markdown: snapshot.bodyMarkdown, saved_revision: 1, updated_at: '' },
  }
}
function changeAnswers(workspace: FeedbackWorkspaceView, update: (answers: QuestionAnswer[]) => QuestionAnswer[]) {
  const envelope = JSON.parse(workspace.draft.document_json!)
  envelope.workbenchState.answers = update(envelope.workbenchState.answers)
  workspace.draft.document_json = JSON.stringify(envelope)
}
const answer = (documentJson: string) => JSON.parse(documentJson).workbenchState.answers[0]

describe('question answer speech writeback', () => {
  it('appends to the selected custom answer and its label while preserving notes and other answers', () => {
    const workspace = fixture()
    const before = structuredClone(workspace)
    const next = applySpeechWriteback(workspace, input)
    const envelope = JSON.parse(next.documentJson)
    expect(answer(next.documentJson)).toEqual({ id: 'q1', value: 'Typed answer\nSpoken answer', label: 'Typed answer\nSpoken answer', wasCustom: true })
    expect(envelope.workbenchState.answers[1]).toEqual(JSON.parse(workspace.draft.document_json!).workbenchState.answers[1])
    expect(envelope.doc).toEqual(JSON.parse(workspace.draft.document_json!).doc)
    expect(envelope.opaque).toEqual({ keep: ['nested'] })
    expect(next.bodyMarkdown).toBe(workspace.draft.body_markdown)
    expect(workspace).toEqual(before)
  })

  it('can start in an empty custom answer without inserting an option index', () => {
    const workspace = fixture('')
    changeAnswers(workspace, (answers) => [{ ...answers[0], index: 2 }, answers[1]])
    expect(answer(applySpeechWriteback(workspace, input).documentJson)).toEqual({ id: 'q1', value: input.text, label: input.text, wasCustom: true })
  })

  it.each(['cleared', 'option', 'duplicate', 'missing-question', 'other-disabled', 'version', 'wrong-type'] as const)
    ('refuses late speech after the target is %s without modifying the draft', (problem) => {
      const workspace = fixture()
      if (problem === 'cleared') changeAnswers(workspace, (answers) => answers.filter((item) => item.id !== 'q1'))
      if (problem === 'option') changeAnswers(workspace, (answers) => [{ id: 'q1', value: 'no', label: 'No', wasCustom: false, index: 2 }, answers[1]])
      if (problem === 'duplicate') changeAnswers(workspace, (answers) => [...answers, answers[0]])
      if (problem === 'missing-question') (workspace.workbench!.data as QuestionsData).questions.shift()
      if (problem === 'other-disabled') (workspace.workbench!.data as QuestionsData).questions[0].allowOther = false
      if (problem === 'version') workspace.workbench!.version = 2
      if (problem === 'wrong-type') workspace.workbench!.type = 'single_choice'
      const before = structuredClone(workspace)
      expect(() => applySpeechWriteback(workspace, input)).toThrow()
      expect(workspace).toEqual(before)
    })

  it('counts non-BMP characters as single scalars and refuses overflow without truncation', () => {
    const workspace = fixture('😀'.repeat(3998))
    const next = applySpeechWriteback(workspace, { ...input, text: '字' })
    expect([...answer(next.documentJson).value]).toHaveLength(4000)
    expect(() => applySpeechWriteback(workspace, { ...input, text: '文字' })).toThrow('text limit')
  })

  it('deduplicates question receipts after reload and rejects operation ids reused for another question', () => {
    const workspace = fixture()
    const first = applySpeechWriteback(workspace, input)
    workspace.draft.document_json = first.documentJson
    const original = structuredClone(workspace)
    expect(applySpeechWriteback(workspace, input)).toEqual(first)
    expect(workspace).toEqual(original)
    changeAnswers(workspace, (answers) => answers.map((item) => ({ ...item, wasCustom: true })))
    const other = { ...input, destination: { kind: 'question_answer' as const, questionId: 'q2', questionLabel: 'Second' } }
    expect(() => applySpeechWriteback(workspace, other)).toThrow('different write')
    expect(() => applySpeechWriteback(workspace, { ...input, text: 'Changed retry payload' })).toThrow('different write')
  })

  it('preserves existing version-1 review receipts and keeps their operation ids reserved', () => {
    const workspace = fixture()
    const envelope = JSON.parse(workspace.draft.document_json!)
    const legacy = { id: 'review-speech', requestId: 'r', annotationId: 'a', field: 'body', sourceVersion: 'v1', text: 'Existing review speech', mergedIds: [] }
    envelope.speechWriteback = { version: 1, operations: [legacy] }
    workspace.draft.document_json = JSON.stringify(envelope)
    const next = applySpeechWriteback(workspace, input)
    expect(JSON.parse(next.documentJson).speechWriteback).toEqual({ version: 1, operations: [legacy, {
      id: input.id, requestId: 'r', text: input.text, mergedIds: [], kind: 'question_answer', questionId: 'q1',
    }] })
    expect(() => applySpeechWriteback(workspace, { ...input, id: legacy.id })).toThrow('different write')
  })

  it('reapplies after a CAS conflict to the latest custom text and deduplicates lost acknowledgements', async () => {
    let current = fixture()
    const save = vi.fn(async (draft: SaveDraftInput): Promise<DraftView> => {
      if (save.mock.calls.length === 1) {
        current = fixture('Concurrent custom typing')
        current.draft.saved_revision = 2
        throw { code: 'DRAFT_CONFLICT', message: 'stale' }
      }
      current.draft = { ...current.draft, document_json: draft.document_json, body_markdown: draft.body_markdown, saved_revision: draft.expected_revision + 1 }
      throw new Error('Acknowledgement lost')
    })
    await expect(writeBackgroundSpeech(input, { load: async () => current, save })).rejects.toThrow('Acknowledgement lost')
    expect(answer(current.draft.document_json!).value).toBe('Concurrent custom typing\nSpoken answer')
    const restored = JSON.parse(JSON.stringify(current)) as FeedbackWorkspaceView
    await expect(writeBackgroundSpeech(input, { load: async () => restored, save })).resolves.toEqual(restored.draft)
    expect(save.mock.calls.map(([draft]) => draft.expected_revision)).toEqual([1, 2])
  })

  it.each(['clear', 'option'] as const)('does not resurrect an answer changed to %s during CAS retry', async (change) => {
    const current = fixture()
    const save = vi.fn(async () => {
      changeAnswers(current, (answers) => change === 'clear' ? answers.slice(1) : [{ id: 'q1', value: 'yes', label: 'Yes', wasCustom: false, index: 1 }, answers[1]])
      current.draft.saved_revision = 2
      throw { code: 'DRAFT_CONFLICT', message: 'stale' }
    })
    await expect(writeBackgroundSpeech(input, { load: async () => current, save })).rejects.toThrow('cleared or changed')
    expect(save).toHaveBeenCalledTimes(1)
  })
})

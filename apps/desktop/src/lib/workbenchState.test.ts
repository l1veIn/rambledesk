import { describe, expect, it } from 'vitest'
import { get } from 'svelte/store'
import { createDraftSession } from './workbench/draftSession'
import { snapshotFeedbackDraftMarkdown, updateFeedbackDraftDocument } from './feedbackDraftDocument'
import { canSubmitWorkbench, readWorkbenchState, withWorkbenchState, workbenchSubmissionIssue } from './workbenchState'
import { writeBackgroundDraftOperation } from './backgroundDraftWriter'
import { workbenchExamples, workbenchPreviewWorkspace } from '../dev/workbenchPreviewFixtures'
import type { QuestionsData, WorkbenchState } from './generated/feedback'
import { resolveWorkbenchPolicy } from './workbenchPolicy'

const state = { type: 'single_choice' as const, selected_option_id: 'compact' }
const snapshot = () => withWorkbenchState(snapshotFeedbackDraftMarkdown(''), state)

describe('workbench submission rules', () => {
  it.each(workbenchExamples)('rejects $type when both input areas are empty', (spec) => {
    expect(workbenchSubmissionIssue(spec, null, ' \n ')).toBe('empty')
    expect(canSubmitWorkbench(spec, null, '')).toBe(false)
  })

  it('accepts notes for Ramble and an independent selection without notes', () => {
    expect(canSubmitWorkbench(workbenchExamples[0], null, 'Feedback')).toBe(true)
    expect(canSubmitWorkbench(undefined, null, 'Legacy feedback')).toBe(true)
    expect(canSubmitWorkbench(workbenchExamples[2], state, '')).toBe(true)
    expect(workbenchSubmissionIssue(workbenchExamples[2], { type: 'single_choice', selected_option_id: null }, '')).toBe('empty')
    expect(workbenchSubmissionIssue(workbenchExamples[2], null, 'Notes')).toBe('incomplete')
  })

  it('distinguishes empty, incomplete and complete questionnaires without treating blank custom text as input', () => {
    const spec = workbenchExamples[1]
    const data = spec.data as QuestionsData
    const answers: WorkbenchState = { type: 'questions', answers: data.questions.map((question) => ({ id: question.id, value: question.options[0].value, label: question.options[0].label, wasCustom: false })) }
    expect(canSubmitWorkbench(spec, answers, '')).toBe(true)
    expect(workbenchSubmissionIssue(spec, { ...answers, answers: answers.answers.slice(0, 1) }, '')).toBe('incomplete')
    expect(workbenchSubmissionIssue(spec, null, 'Notes')).toBe('incomplete')
    const blank: WorkbenchState = { type: 'questions', answers: [{ id: 'audience', value: ' \n ', label: '', wasCustom: true }] }
    expect(workbenchSubmissionIssue(spec, blank, '')).toBe('empty')
    expect(workbenchSubmissionIssue(spec, { ...answers, answers: [...answers.answers, answers.answers[0]] }, 'Notes')).toBe('incomplete')
  })

  it('uses the same unsupported decision for rendering and submission, including future Ramble versions', () => {
    for (const spec of [
      { ...workbenchExamples[0], version: 99 },
      { ...workbenchExamples[2], type: 'future_workbench' },
      { ...workbenchExamples[2], data: { options: null } },
    ]) {
      expect(resolveWorkbenchPolicy(spec)).toBeNull()
      expect(workbenchSubmissionIssue(spec, state, 'Notes')).toBe('unsupported')
    }
  })
})

describe('workbench answer persistence', () => {
  it('restores answers, preserves them while notes change, and keeps local choices after a stale save', () => {
    const session = createDraftSession()
    const stored = snapshot()
    session.adopt({ document_json: stored.documentJson, body_markdown: '', saved_revision: 1, updated_at: 'now' })
    expect(readWorkbenchState(session.snapshot().documentJson)).toEqual(state)
    expect(session.isDirty()).toBe(false)
    session.edit(snapshotFeedbackDraftMarkdown('Notes only'))
    const saving = session.snapshot()
    expect(readWorkbenchState(saving.documentJson)).toEqual(state)
    session.edit(withWorkbenchState(saving, { type: 'single_choice', selected_option_id: 'spacious' }))
    session.acceptSaved(saving, 2)
    expect(get(session).dirty).toBe(true)
    expect(readWorkbenchState(session.snapshot().documentJson)).toEqual({ type: 'single_choice', selected_option_id: 'spacious' })
    session.adopt({ document_json: null, body_markdown: 'Another request', saved_revision: 0, updated_at: null })
    expect(readWorkbenchState(session.snapshot().documentJson)).toBeNull()
  })

  it('preserves answers during background capture and programmatic document transformations', async () => {
    const workspace = workbenchPreviewWorkspace(2)
    workspace.draft.document_json = snapshot().documentJson
    const saved = await writeBackgroundDraftOperation(workspace.request.request_id,
      { kind: 'appendClipboardText', text: 'Background notes', label: '', action: null }, {
        load: async () => workspace,
        save: async (input) => ({ ...workspace.draft, document_json: input.document_json, body_markdown: input.body_markdown, saved_revision: input.expected_revision + 1 }),
      })
    expect(saved.body_markdown).toContain('Background notes')
    expect(readWorkbenchState(saved.document_json)).toEqual(state)
    const transformed = updateFeedbackDraftDocument({ documentJson: saved.document_json!, bodyMarkdown: saved.body_markdown }, (doc) => ({ ...doc, content: [] }))
    expect(transformed.bodyMarkdown).toBe('')
    expect(readWorkbenchState(transformed.documentJson)).toEqual(state)
  })
})

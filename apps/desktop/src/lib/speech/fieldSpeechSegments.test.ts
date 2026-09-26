import { describe, expect, it } from 'vitest'
import type { DocumentReviewData, QuestionsData, ReviewAnnotation } from '../generated/feedback'
import { snapshotFeedbackDraftDocument, type FeedbackDraftSnapshot } from '../feedbackDraftDocument'
import { applySpeechWriteback, type SpeechWriteInput, type SpeechWriteWorkspace } from './speechWriteback'
import type { SpeechTarget } from './speechTargets'
import { sameWorkbenchFieldIdentity } from '../workbenchFields'
import { captureFieldTidy, replaceFieldTidy } from './fieldSpeechText'
import { applyFieldSpeechCleanup, collectFieldSpeechSegments, fieldSpeechSegmentCounts, fieldSpeechSegmentsForTarget, reconcileFieldSpeechSegments,
  type FieldSpeechCleanupReplacement } from './fieldSpeechSegments'

function fixture(type: 'questions' | 'document_review' = 'questions'): SpeechWriteWorkspace {
  const snapshot = snapshotFeedbackDraftDocument({ type: 'doc', content: [{ type: 'paragraph', content: [{ type: 'text', text: 'Keep notes' }] }] })
  const annotation: ReviewAnnotation = { id: 'a', paragraph_id: 'p', start: 1, end: 2, quote: '😀',
    kind: 'suggestion', body: 'Typed comment', replacement: 'Typed wording' }
  return { request: { request_id: 'r', status: 'in_progress' },
    workbench: type === 'questions' ? { type, version: 1, data: { questions: ['q1', 'q2'].map((id) => ({ id,
      prompt: id, allowOther: true, options: [{ value: 'yes', label: 'Yes' }, { value: 'no', label: 'No' }] })) } }
      : { type, version: 1, data: { title: 'Script', source_version: 'v1', paragraphs: [{ id: 'p', text: 'A😀B' }] } },
    draft: { document_json: JSON.stringify({ ...JSON.parse(snapshot.documentJson), opaque: { keep: ['nested'] },
      workbenchState: type === 'questions' ? { type, answers: ['q1', 'q2'].map((id) => ({ id, value: 'Typed😀', label: 'Typed😀', wasCustom: true })) }
        : { type, annotations: [annotation], paragraph_marks: [], verdict: null } }), body_markdown: snapshot.bodyMarkdown },
  }
}
const question = (questionId = 'q1'): SpeechTarget => ({ requestId: 'r', requestTitle: 'Questions',
  destination: { kind: 'question_answer', questionId, questionLabel: questionId } })
const review = (field: 'body' | 'replacement'): SpeechTarget => ({ requestId: 'r', requestTitle: 'Review',
  destination: { kind: 'review_annotation', annotationId: 'a', field, sourceVersion: 'v1', paragraphLabel: 'Opening' } })
const snapshotOf = (w: SpeechWriteWorkspace): FeedbackDraftSnapshot => ({ documentJson: w.draft.document_json!, bodyMarkdown: w.draft.body_markdown })
const adopt = (w: SpeechWriteWorkspace, s: FeedbackDraftSnapshot) => { w.draft = { document_json: s.documentJson, body_markdown: s.bodyMarkdown } }
const envelope = (w: SpeechWriteWorkspace) => JSON.parse(w.draft.document_json!)
function speech(w: SpeechWriteWorkspace, id: string, text: string, target = question(), cleanupState?: 'pending' | 'cleaned') {
  const input: SpeechWriteInput = { ...target, id, text, cleanupState }
  adopt(w, applySpeechWriteback(w, input))
  return input
}
function edit(w: SpeechWriteWorkspace, change: (envelope: any) => void, reconcile = true) {
  const previous = snapshotOf(w), next = JSON.parse(previous.documentJson)
  change(next)
  const snapshot = { ...previous, documentJson: JSON.stringify(next) }
  adopt(w, reconcile ? reconcileFieldSpeechSegments(previous, snapshot) : snapshot)
}
const replacements = (w: SpeechWriteWorkspace): FieldSpeechCleanupReplacement[] => collectFieldSpeechSegments(w)
  .map(({ segmentId, text, target }) => ({ segmentId, originalText: text, nextText: `Clean ${segmentId}`, target }))

describe('structured field speech provenance', () => {
  it('projects only untouched speech for the selected field and keeps cleaned provenance available', () => {
    const w = fixture()
    speech(w, 'pending', 'Raw speech')
    speech(w, 'cleaned', 'Clean speech', question(), 'cleaned')
    speech(w, 'other', 'Other answer', question('q2'))
    expect(fieldSpeechSegmentsForTarget(snapshotOf(w), question()).map(({ segmentId, state }) => ({ segmentId, state })))
      .toEqual([{ segmentId: 'pending', state: 'pending' }, { segmentId: 'cleaned', state: 'cleaned' }])
    edit(w, (e) => { e.workbenchState.answers[0].value = e.workbenchState.answers[0].value.replace('Raw speech', 'Manual edit') })
    expect(fieldSpeechSegmentsForTarget(snapshotOf(w), question()).map(({ segmentId }) => segmentId)).toEqual(['cleaned'])
    expect(fieldSpeechSegmentsForTarget(snapshotOf(w), { ...question(), requestId: 'other' })).toEqual([])
    edit(w, (e) => { e.workbenchState.answers[0].value = 'Externally replaced without reconciliation' }, false)
    expect(fieldSpeechSegmentsForTarget(snapshotOf(w), question())).toEqual([])
  })

  it('tracks only new spoken spans, preserves Unicode scalar offsets, and never duplicates an acknowledged retry', () => {
    const w = fixture()
    expect(collectFieldSpeechSegments(w)).toEqual([])
    const input = speech(w, 's1', '嗯，回答😀')
    const before = snapshotOf(w)
    expect(collectFieldSpeechSegments(w)).toMatchObject([{ segmentId: 's1', start: 7, end: 12, text: '嗯，回答😀', state: 'pending' }])
    expect(fieldSpeechSegmentCounts(before, question())).toEqual({ pending: 1, cleaned: 0 })
    expect(applySpeechWriteback(w, input)).toEqual(before)
    expect(envelope(w).fieldSpeechSegments.segments).toHaveLength(1)
    speech(w, 's2', 'Already tidy', question(), 'cleaned')
    expect(collectFieldSpeechSegments(w).map((s) => s.segmentId)).toEqual(['s1'])
    expect(fieldSpeechSegmentCounts(snapshotOf(w), question())).toEqual({ pending: 1, cleaned: 1 })
  })

  it('cleans independent answers in one snapshot while retaining notes, receipts, and opaque data', () => {
    const w = fixture()
    speech(w, 's1', 'First raw', question())
    speech(w, 's2', 'Second raw', question('q2'))
    const before = envelope(w)
    const result = applyFieldSpeechCleanup(w, replacements(w))
    expect(result.applied).toEqual(['s1', 's2'])
    expect(result.skipped).toEqual([])
    const after = JSON.parse(result.snapshot.documentJson)
    expect(after.workbenchState.answers.map((a: any) => a.value)).toEqual(['Typed😀\nClean s1', 'Typed😀\nClean s2'])
    expect(after.workbenchState.answers.every((a: any) => a.label === a.value && a.wasCustom)).toBe(true)
    expect(after.speechWriteback).toEqual(before.speechWriteback)
    expect(after.opaque).toEqual(before.opaque)
    expect(after.doc).toEqual(before.doc)
    expect(result.snapshot.bodyMarkdown).toBe(w.draft.body_markdown)
    expect(envelope(w)).toEqual(before)
  })

  it('keeps review comment and replacement spans separate without modifying source anchors or paragraph marks', () => {
    const w = fixture('document_review')
    speech(w, 'body', '嗯，说明', review('body'))
    speech(w, 'wording', '呃，新稿', review('replacement'))
    const before = envelope(w)
    const result = applyFieldSpeechCleanup(w, replacements(w))
    const after = JSON.parse(result.snapshot.documentJson)
    expect(result.applied).toEqual(['body', 'wording'])
    expect(after.workbenchState.annotations[0]).toEqual({ ...before.workbenchState.annotations[0],
      body: 'Typed comment\nClean body', replacement: 'Typed wording\nClean wording' })
    expect(after.workbenchState.paragraph_marks).toEqual(before.workbenchState.paragraph_marks)
    expect(after.speechWriteback).toEqual(before.speechWriteback)
  })

  it('keeps earlier body speech pending and tidies it after the comment gains suggested wording', () => {
    const w = fixture('document_review')
    edit(w, (e) => Object.assign(e.workbenchState.annotations[0], { kind: 'comment', replacement: null }))
    speech(w, 'body', 'Raw comment', review('body'))
    const wanted = replacements(w)
    const originalIdentity = collectFieldSpeechSegments(w)[0].identity
    edit(w, (e) => Object.assign(e.workbenchState.annotations[0], { kind: 'suggestion', replacement: 'Proposed wording' }))
    expect(collectFieldSpeechSegments(w).map((s) => s.segmentId)).toEqual(['body'])
    expect(fieldSpeechSegmentCounts(snapshotOf(w), review('body'))).toEqual({ pending: 1, cleaned: 0 })

    const before = envelope(w)
    const result = applyFieldSpeechCleanup(w, wanted)
    expect(result.applied).toEqual(['body'])
    expect(result.skipped).toEqual([])
    adopt(w, result.snapshot)
    expect(envelope(w).workbenchState.annotations[0]).toEqual({ ...before.workbenchState.annotations[0], body: 'Typed comment\nClean body' })
    expect(envelope(w).fieldSpeechSegments.segments[0].identity).toBe(originalIdentity)
    expect(fieldSpeechSegmentCounts(snapshotOf(w), review('body'))).toEqual({ pending: 0, cleaned: 1 })
    speech(w, 'later', 'Another thought', review('body'))
    expect(collectFieldSpeechSegments(w).map((s) => s.segmentId)).toEqual(['later'])
  })

  it('accepts a body tidy captured before adding suggested wording while preserving concurrent wording', () => {
    const w = fixture('document_review')
    edit(w, (e) => Object.assign(e.workbenchState.annotations[0], { kind: 'comment', replacement: null }))
    const captured = captureFieldTidy(w, review('body'))
    edit(w, (e) => Object.assign(e.workbenchState.annotations[0], { kind: 'suggestion', replacement: 'Concurrent wording' }))
    const snapshot = replaceFieldTidy(w, captured, captured.original, 'Cleaned comment')
    expect(JSON.parse(snapshot.documentJson).workbenchState.annotations[0]).toEqual({
      ...envelope(w).workbenchState.annotations[0], body: 'Cleaned comment',
    })
    edit(w, (e) => { e.workbenchState.annotations[0].body = 'Manual body edit' })
    expect(() => replaceFieldTidy(w, captured, captured.original, 'Late model result')).toThrow('edited after tidying')
  })

  it('limits identity compatibility to the same body upgrading from a comment to a suggestion', () => {
    const w = fixture('document_review')
    const suggested = captureFieldTidy(w, review('body')).identity
    edit(w, (e) => Object.assign(e.workbenchState.annotations[0], { kind: 'comment', replacement: null }))
    const captured = captureFieldTidy(w, review('body')).identity
    expect(sameWorkbenchFieldIdentity(captured, suggested, review('body'))).toBe(true)
    expect(sameWorkbenchFieldIdentity(suggested, captured, review('body'))).toBe(false)
    expect(sameWorkbenchFieldIdentity(captured, suggested, review('replacement'))).toBe(false)
    expect(sameWorkbenchFieldIdentity(captured, suggested, question())).toBe(false)
    expect(sameWorkbenchFieldIdentity('invalid legacy identity', suggested, review('body'))).toBe(false)
    for (const change of [{ id: 'another' }, { paragraphId: 'another' }, { start: 0 }, { end: 3 }, { quote: 'B' }]) {
      expect(sameWorkbenchFieldIdentity(captured, JSON.stringify({ ...JSON.parse(suggested), ...change }), review('body'))).toBe(false)
    }
  })

  it('still rejects a changed anchor or downgrade after recording comment speech', () => {
    const w = fixture('document_review')
    speech(w, 'body', 'Suggestion speech', review('body'))
    const wanted = replacements(w)
    edit(w, (e) => Object.assign(e.workbenchState.annotations[0], { kind: 'comment', replacement: null }))
    expect(collectFieldSpeechSegments(w)).toEqual([])
    expect(applyFieldSpeechCleanup(w, wanted).skipped).toEqual(['body'])
    speech(w, 'comment', 'New comment speech', review('body'))
    edit(w, (e) => Object.assign(e.workbenchState.annotations[0], {
      kind: 'suggestion', replacement: '', start: null, end: null, quote: null,
    }))
    expect(collectFieldSpeechSegments(w)).toEqual([])
  })

  it('moves later spans as earlier speech shrinks and retains subsequent new speech as pending', () => {
    const w = fixture()
    speech(w, 's1', 'Long first raw')
    speech(w, 's2', 'Second raw')
    const wanted = replacements(w).map((s, i) => ({ ...s, nextText: i === 0 ? '' : 'Second cleaned' }))
    speech(w, 's3', 'New speech during tidy')
    const result = applyFieldSpeechCleanup(w, wanted)
    adopt(w, result.snapshot)
    expect(result.applied).toEqual(['s1', 's2'])
    expect(envelope(w).workbenchState.answers[0].value).toBe('Typed😀\n\nSecond cleaned\nNew speech during tidy')
    expect(collectFieldSpeechSegments(w).map((s) => s.segmentId)).toEqual(['s3'])
    expect(fieldSpeechSegmentCounts(snapshotOf(w), question())).toEqual({ pending: 1, cleaned: 2 })
  })

  it('moves untouched spans around manual prefix and suffix edits, but removes edited speech from pending counts', () => {
    const w = fixture()
    speech(w, 's1', 'Speech one')
    speech(w, 's2', 'Speech two')
    edit(w, (e) => { e.workbenchState.answers[0].value = `😀Prefix\n${e.workbenchState.answers[0].value}` })
    expect(collectFieldSpeechSegments(w).map((s) => s.start)).toEqual([15, 26])
    edit(w, (e) => { e.workbenchState.answers[0].value += '\nManual suffix' })
    expect(collectFieldSpeechSegments(w)).toHaveLength(2)
    edit(w, (e) => { e.workbenchState.answers[0].value = e.workbenchState.answers[0].value.replace('Speech one', 'My edited answer') })
    expect(collectFieldSpeechSegments(w).map((s) => s.segmentId)).toEqual(['s2'])
    expect(envelope(w).fieldSpeechSegments.segments[0].state).toBe('edited')
    expect(fieldSpeechSegmentCounts(snapshotOf(w), question())).toEqual({ pending: 1, cleaned: 0 })
  })

  it.each(['clear', 'option'] as const)('invalidates provenance on %s, even if identical custom text is later re-created', (change) => {
    const w = fixture()
    speech(w, 's1', 'Original speech')
    const oldAnswer = envelope(w).workbenchState.answers[0]
    edit(w, (e) => { e.workbenchState.answers = change === 'clear' ? [] : [{ ...oldAnswer, wasCustom: false, value: 'yes', label: 'Yes' }] })
    edit(w, (e) => { e.workbenchState.answers = [oldAnswer] })
    expect(collectFieldSpeechSegments(w)).toEqual([])
    expect(envelope(w).fieldSpeechSegments.segments[0].state).toBe('edited')
  })

  it('skips speech edited during model generation while applying unchanged speech in another answer', () => {
    const w = fixture()
    speech(w, 's1', 'First raw')
    speech(w, 's2', 'Second raw', question('q2'))
    const wanted = replacements(w)
    edit(w, (e) => { e.workbenchState.answers[0].value = 'Human revision'; e.opaque.concurrent = true })
    const result = applyFieldSpeechCleanup(w, wanted)
    expect(result.applied).toEqual(['s2'])
    expect(result.skipped).toEqual(['s1'])
    const after = JSON.parse(result.snapshot.documentJson)
    expect(after.workbenchState.answers[0].value).toBe('Human revision')
    expect(after.opaque.concurrent).toBe(true)
  })

  it.each(['closed', 'request', 'contract', 'target', 'original', 'overflow'] as const)('refuses a stale or invalid %s replacement', (change) => {
    const w = fixture()
    speech(w, 's1', 'First raw')
    const wanted = replacements(w)
    if (change === 'closed') w.request.status = 'completed'
    if (change === 'request') w.request.request_id = 'other'
    if (change === 'contract') (w.workbench!.data as QuestionsData).questions[0].prompt = 'Changed question'
    if (change === 'target') wanted[0].target = question('q2')
    if (change === 'original') wanted[0].originalText = 'Not the original'
    if (change === 'overflow') wanted[0].nextText = '字'.repeat(4001)
    const before = snapshotOf(w)
    const result = applyFieldSpeechCleanup(w, wanted)
    expect(result.applied).toEqual([])
    expect(result.skipped).toEqual(['s1'])
    expect(result.snapshot).toEqual(before)
  })

  it.each(['deleted', 'source', 'anchor'] as const)('protects review speech after its %s changes', (change) => {
    const w = fixture('document_review')
    speech(w, 's1', 'Review raw', review('body'))
    const wanted = replacements(w)
    if (change === 'source') (w.workbench!.data as DocumentReviewData).source_version = 'v2'
    else edit(w, (e) => {
      if (change === 'deleted') e.workbenchState.annotations = []
      else e.workbenchState.annotations[0].quote = 'B'
    })
    expect(applyFieldSpeechCleanup(w, wanted).applied).toEqual([])
  })

  it('preserves unsupported metadata and does not guess legacy speech offsets', () => {
    const w = fixture()
    edit(w, (e) => { e.fieldSpeechSegments = { version: 2, future: ['opaque'] } }, false)
    speech(w, 's1', 'New raw speech')
    expect(envelope(w).fieldSpeechSegments).toEqual({ version: 2, future: ['opaque'] })
    expect(collectFieldSpeechSegments(w)).toEqual([])
  })

  it('guards unanchored comments against same-version source edits without storing unrelated paragraphs', () => {
    const w = fixture('document_review')
    const data = w.workbench!.data as DocumentReviewData
    data.paragraphs.push({ id: 'other', text: 'Unrelated large original document section' })
    edit(w, (e) => Object.assign(e.workbenchState.annotations[0], { start: null, end: null, quote: null }))
    speech(w, 's1', 'Whole paragraph comment', review('body'))
    const wanted = replacements(w)
    const contract = collectFieldSpeechSegments(w)[0].contract
    expect(contract).toContain('A😀B')
    expect(contract).not.toContain(data.paragraphs[1].text)
    data.paragraphs[1].text = 'An unrelated paragraph changed'
    expect(collectFieldSpeechSegments(w)).toHaveLength(1)
    data.paragraphs[0].text = 'Same version, changed original paragraph'
    expect(collectFieldSpeechSegments(w)).toHaveLength(0)
    expect(applyFieldSpeechCleanup(w, wanted).skipped).toEqual(['s1'])
  })

  it('does not invalidate question speech when another question changes', () => {
    const w = fixture()
    speech(w, 's1', 'Answer first question')
    const data = w.workbench!.data as QuestionsData
    data.questions[1].prompt = 'A changed second question'
    expect(collectFieldSpeechSegments(w)).toHaveLength(1)
    data.questions[0].prompt = 'A changed first question'
    expect(collectFieldSpeechSegments(w)).toHaveLength(0)
  })
})

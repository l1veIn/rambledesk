import { describe, expect, it } from 'vitest'
import type { InputWriteWorkspace } from '../../../domain/inputTarget'
import { workbenchFieldTarget } from '../../../domain/inputTarget'
import { applyInputTextWriteback } from '../../../inputTextWriteback'
import { snapshotFeedbackDraftMarkdown, type FeedbackDraftSnapshot } from '../../../feedbackDraftDocument'
import { readWorkbenchState, withWorkbenchState } from '../../../workbenchState'
import { canSubmitWorkbench } from '../../../workbenchState'
import { removeWorkbenchAttachmentReferences } from '../../../input/fieldAttachmentText'
import { applySpeechWriteback } from '../../../speech/speechWriteback'
import { applyFieldSpeechCleanup, collectFieldSpeechSegments } from '../../../speech/fieldSpeechSegments'
import { captureFieldTidy, replaceFieldTidy } from '../../../speech/fieldSpeechText'
import { getWorkbenchDefinition } from '../registry'
import { ratingMaterialVersion, ratingReviewDefinition } from './definition'

function workspace(): InputWriteWorkspace {
  const spec = structuredClone(ratingReviewDefinition.examples![0].spec)
  const snapshot = withWorkbenchState(snapshotFeedbackDraftMarkdown('Supplementary feedback'), { type: 'rating_review', score: null, note: 'Typed opinion' })
  return { request: { request_id: 'rating-request', status: 'in_progress' }, workbench: spec,
    draft: { document_json: JSON.stringify({ ...JSON.parse(snapshot.documentJson), futureMetadata: { preserve: true } }), body_markdown: snapshot.bodyMarkdown } }
}
function target(current: InputWriteWorkspace) {
  return workbenchFieldTarget({ requestId: current.request.request_id, requestTitle: 'Rating trial' }, {
    workbenchType: 'rating_review', version: 1, field: 'opinion', entityId: 'review',
    sourceVersion: ratingMaterialVersion(current.workbench!.data), label: 'Opinion',
  })
}
function updated(current: InputWriteWorkspace, snapshot: FeedbackDraftSnapshot): InputWriteWorkspace {
  return { ...current, draft: { document_json: snapshot.documentJson, body_markdown: snapshot.bodyMarkdown } }
}

describe('rating review through registered field capabilities', () => {
  it('uses the registered module for paste, attachment references and removal without relaxing the required score', () => {
    expect(getWorkbenchDefinition('rating_review')).toBe(ratingReviewDefinition)
    const current = workspace(), destination = target(current)
    const input = { target: destination, text: '![Screenshot](attachment://shot)', id: 'attachment-1' }
    const snapshot = applyInputTextWriteback(current, input)
    expect(readWorkbenchState(snapshot.documentJson)).toEqual({ type: 'rating_review', score: null, note: 'Typed opinion\n![Screenshot](attachment://shot)' })
    expect(applyInputTextWriteback(updated(current, snapshot), input)).toEqual(snapshot)
    expect(canSubmitWorkbench(current.workbench, readWorkbenchState(snapshot.documentJson), snapshot.bodyMarkdown)).toBe(false)
    const removed = removeWorkbenchAttachmentReferences(snapshot, 'shot')
    expect(readWorkbenchState(removed.documentJson)).toEqual({ type: 'rating_review', score: null, note: 'Typed opinion\n' })
    expect(removed.bodyMarkdown).toBe('Supplementary feedback')
    expect(JSON.parse(removed.documentJson).futureMetadata).toEqual({ preserve: true })
  })

  it('accepts speech with receipts and guarded cleanup through the same opinion adapter', () => {
    const current = workspace(), destination = target(current)
    const speech = { ...destination, id: 'speech-1', text: '嗯，这个设计清楚🙂' }
    const spoken = applySpeechWriteback(current, speech)
    const latest = updated(current, spoken)
    expect(applySpeechWriteback(latest, speech)).toEqual(spoken)
    expect(collectFieldSpeechSegments(latest)).toHaveLength(1)
    const cleaned = applyFieldSpeechCleanup(latest, [{ target: destination, segmentId: 'speech-1', originalText: speech.text, nextText: '这个设计清楚🙂' }])
    expect(cleaned.applied).toEqual(['speech-1'])
    expect(readWorkbenchState(cleaned.snapshot.documentJson)).toMatchObject({ note: 'Typed opinion\n这个设计清楚🙂' })
    expect(cleaned.snapshot.bodyMarkdown).toBe('Supplementary feedback')
    expect(JSON.parse(cleaned.snapshot.documentJson).futureMetadata).toEqual({ preserve: true })
  })

  it('keeps delayed input fixed to the original material and protects edits made while tidy is pending', () => {
    const current = workspace(), destination = target(current)
    const capture = captureFieldTidy(current, destination)
    const edited = withWorkbenchState({ documentJson: current.draft.document_json!, bodyMarkdown: current.draft.body_markdown },
      { type: 'rating_review', score: 4, note: 'Edited while waiting' })
    expect(() => replaceFieldTidy(updated(current, edited), capture, 'Typed opinion', 'Cleaned opinion')).toThrow('edited')
    const changedMaterial = { ...current, workbench: { ...current.workbench!, data: { title: 'Changed source', material: 'Other evidence' } } }
    const closed = { ...current, request: { ...current.request, status: 'cancelled' as const } }
    const missing = updated(current, withWorkbenchState({ documentJson: current.draft.document_json!, bodyMarkdown: current.draft.body_markdown },
      { type: 'questions', answers: [] }))
    for (const unavailable of [changedMaterial, closed, missing]) {
      expect(() => applyInputTextWriteback(unavailable, { target: destination, id: 'late-paste', text: 'Never redirect' })).toThrow()
      expect(() => applySpeechWriteback(unavailable, { ...destination, id: 'late-speech', text: 'Never redirect' })).toThrow()
      expect(unavailable.draft.body_markdown).toBe('Supplementary feedback')
    }
  })
})

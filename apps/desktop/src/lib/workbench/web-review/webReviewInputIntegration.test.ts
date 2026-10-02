import { describe, expect, it } from 'vitest'
import type { WebReviewAnnotation, WorkbenchSpec } from '../../generated/feedback'
import type { InputTarget, InputWriteWorkspace } from '../../domain/inputTarget'
import { normalizeInputTarget, sameInputTarget } from '../../domain/inputTarget'
import { snapshotFeedbackDraftMarkdown, updateFeedbackDraftState } from '../../feedbackDraftDocument'
import { applyInputTextWriteback } from '../../inputTextWriteback'
import { readWorkbenchState, canSubmitWorkbench } from '../../workbenchState'
import { workbenchIsReadOnly } from '../definitions/registry'
import { validWebReviewInput } from '../definitions/web_review/input'
import { applySpeechWriteback } from '../../speech/speechWriteback'
import { collectFieldSpeechSegments, applyFieldSpeechCleanup } from '../../speech/fieldSpeechSegments'
import { captureFieldTidy, replaceFieldTidy } from '../../speech/fieldSpeechText'
import { removeWorkbenchAttachmentReferences } from '../fields/attachmentReferences'

const annotation: WebReviewAnnotation = {
  id: 'button-note', page_url: 'http://localhost:5173/pricing', viewport: { width: 1280, height: 800 },
  element: { selector: '#signup', tag_name: 'button', text: '免费开始', rect: { x: 80, y: 320, width: 140, height: 42 } },
  body: '现有意见',
}
const spec: WorkbenchSpec = { type: 'web_review', version: 1, data: {
  title: '首页', source_version: 'homepage-v1', url: 'http://localhost:5173/', viewport: { width: 1280, height: 800 },
} }
const target: InputTarget = { requestId: 'web-request', requestTitle: '网页评审',
  destination: { kind: 'web_review_annotation', annotationId: annotation.id, elementLabel: '免费开始' } }

function workspace(): InputWriteWorkspace {
  const snapshot = updateFeedbackDraftState(snapshotFeedbackDraftMarkdown('整体意见'), { type: 'web_review', annotations: [structuredClone(annotation)] })
  return { request: { request_id: target.requestId, status: 'in_progress' }, workbench: structuredClone(spec),
    draft: { document_json: snapshot.documentJson, body_markdown: snapshot.bodyMarkdown } }
}

describe('web review through shared input and draft contracts', () => {
  it('restores explicit destinations without redirecting unavailable fields into overall feedback', () => {
    expect(normalizeInputTarget(JSON.parse(JSON.stringify(target)))).toEqual(target)
    expect(sameInputTarget(target, { ...target, requestTitle: 'Renamed request' })).toBe(true)
    expect(sameInputTarget(target, { ...target, destination: { kind: 'web_review_annotation', annotationId: 'another', elementLabel: 'Another' } })).toBe(false)
    const current = workspace()
    current.draft.document_json = updateFeedbackDraftState(snapshotFeedbackDraftMarkdown('整体意见'), { type: 'web_review', annotations: [] }).documentJson
    expect(() => applyInputTextWriteback(current, { target, id: 'late-paste', text: '迟到的意见' })).toThrow('target comment')
    expect(current.draft.body_markdown).toBe('整体意见')
  })

  it('saves empty in-progress comments but prevents overall feedback from bypassing them', () => {
    const current = workspace()
    const pending = { type: 'web_review' as const, annotations: [{ ...annotation, body: '' }] }
    const snapshot = updateFeedbackDraftState(snapshotFeedbackDraftMarkdown('整体意见'), pending)
    expect(readWorkbenchState(snapshot.documentJson)).toEqual(pending)
    expect(canSubmitWorkbench(spec, pending, '整体意见')).toBe(false)
    expect(canSubmitWorkbench(spec, null, '整体意见')).toBe(true)
    expect(canSubmitWorkbench(spec, { type: 'web_review', annotations: [annotation] }, '')).toBe(true)
    expect(current.workbench).toEqual(spec)
  })

  it('retains page anchors and overall notes while retrying speech and cleaning the same field', () => {
    const current = workspace()
    const write = { ...target, id: 'spoken-note', text: '嗯，需要说明无需信用卡。' }
    const first = applySpeechWriteback(current, write)
    current.draft = { document_json: first.documentJson, body_markdown: first.bodyMarkdown }
    expect(applySpeechWriteback(current, write)).toEqual(first)
    const segments = collectFieldSpeechSegments(current)
    expect(segments).toHaveLength(1)
    const cleaned = applyFieldSpeechCleanup(current, [{ target, segmentId: write.id, originalText: write.text, nextText: '需要说明无需信用卡。' }])
    expect(cleaned.applied).toEqual([write.id])
    expect(readWorkbenchState(cleaned.snapshot.documentJson)).toMatchObject({ annotations: [{ ...annotation, body: '现有意见\n需要说明无需信用卡。' }] })
    expect(cleaned.snapshot.bodyMarkdown).toBe('整体意见')
    expect(() => applySpeechWriteback(current, { ...write, text: '不同的转写' })).toThrow('different write')
  })

  it('rejects asynchronous tidy results when the selected page state has changed', () => {
    const current = workspace()
    const capture = captureFieldTidy(current, target)
    const snapshot = updateFeedbackDraftState(snapshotFeedbackDraftMarkdown('整体意见'), { type: 'web_review',
      annotations: [{ ...annotation, viewport: { width: 390, height: 844 } }] })
    current.draft.document_json = snapshot.documentJson
    expect(() => replaceFieldTidy(current, capture, annotation.body, '整理后的意见')).toThrow('changed after tidying')
    expect(readWorkbenchState(current.draft.document_json)).toMatchObject({ annotations: [{ viewport: { width: 390, height: 844 }, body: annotation.body }] })
  })

  it('imports and removes attachment references from the same comment without losing its anchor', () => {
    const current = workspace()
    const text = '![截图](attachment://screen-1)'
    const pasted = applyInputTextWriteback(current, { target, id: 'attachment-write', text })
    current.draft = { document_json: pasted.documentJson, body_markdown: pasted.bodyMarkdown }
    expect(applyInputTextWriteback(current, { target, id: 'attachment-write', text })).toEqual(pasted)
    const removed = removeWorkbenchAttachmentReferences(pasted, 'screen-1')
    expect(readWorkbenchState(removed.documentJson)).toEqual({ type: 'web_review', annotations: [{ ...annotation, body: `${annotation.body}\n` }] })
    expect(removed.bodyMarkdown).toBe('整体意见')
  })

  it.each(['javascript:alert(1)', 'file:///tmp/page', 'https://name:password@example.test/', 'https://example.test/\n'])('keeps unsafe URL %s read-only', (url) => {
    const bad = { ...spec, data: { ...spec.data, url } }
    expect(validWebReviewInput(bad.data)).toBe(false)
    expect(workbenchIsReadOnly(bad)).toBe(true)
    expect(canSubmitWorkbench(bad, null, 'Notes')).toBe(false)
  })
})

import { applyDraftOperation, type DraftOperation } from '../draftOperations'
import { isWorkbenchFieldDestination, sameInputTarget, type InputWriteWorkspace, type WorkbenchFieldDestination } from '../domain/inputTarget'
import { decodeFeedbackDraftEnvelope, restoreFeedbackDraftSnapshot, updateFeedbackDraftDocument, type FeedbackDraftSnapshot } from '../feedbackDraftDocument'
import { resolveWorkbenchDefinition } from '../workbench/definitions/registry'
import { appendWorkbenchField, readWorkbenchField } from '../workbenchFields'
import type { SpeechTarget } from './speechTargets'
import { recordFieldSpeechSegment } from './fieldSpeechSegments'

export type SpeechWriteInput = SpeechTarget & {
  id: string
  text: string
  cleanupState?: 'pending' | 'cleaned'
  mergedIds?: string[]
}

export type SpeechWriteWorkspace = InputWriteWorkspace

type WriteReceiptBase = { id: string; requestId: string; text: string; mergedIds: string[] }
type ReviewWriteReceipt = WriteReceiptBase & {
  // Existing version-1 review receipts have no kind field; keep that wire shape.
  kind?: undefined; annotationId: string; field: 'body' | 'replacement'; sourceVersion: string
}
type QuestionWriteReceipt = WriteReceiptBase & { kind: 'question_answer'; questionId: string }
type WebReviewWriteReceipt = WriteReceiptBase & { kind: 'web_review_annotation'; annotationId: string }
type FieldWriteReceipt = WriteReceiptBase & { kind: 'workbench_field'; destination: WorkbenchFieldDestination }
type WriteReceipt = ReviewWriteReceipt | QuestionWriteReceipt | WebReviewWriteReceipt | FieldWriteReceipt
const record = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value)

/** Keep the existing document operation and its speech segment deduplication. */
export function speechDocumentOperation(input: SpeechWriteInput): DraftOperation {
  if (input.destination.kind !== 'document') throw new Error('This speech target is not a document.')
  return { kind: 'appendSpeech', segmentId: input.id, text: input.text, action: input.destination.action,
    ...(input.cleanupState === 'cleaned' ? { cleanupState: 'cleaned' } : {}) }
}

function isWriteReceipt(value: unknown): value is WriteReceipt {
  if (!record(value) || typeof value.id !== 'string' || typeof value.requestId !== 'string' || typeof value.text !== 'string'
    || !Array.isArray(value.mergedIds) || !value.mergedIds.every((id) => typeof id === 'string')) return false
  if (value.kind === 'question_answer') return typeof value.questionId === 'string'
  if (value.kind === 'web_review_annotation') return typeof value.annotationId === 'string'
  if (value.kind === 'workbench_field') return isWorkbenchFieldDestination(value.destination)
  return value.kind === undefined && typeof value.annotationId === 'string'
    && (value.field === 'body' || value.field === 'replacement') && typeof value.sourceVersion === 'string'
}

function readReceipts(value: unknown): WriteReceipt[] {
  if (value === undefined) return []
  if (!record(value) || value.version !== 1 || !Array.isArray(value.operations) || !value.operations.every(isWriteReceipt)) {
    throw new Error('The speech write history is not supported. Your words have been preserved.')
  }
  return value.operations
}

function sameReceiptDestination(first: WriteReceipt, second: WriteReceipt): boolean {
  if (first.kind === 'workbench_field' || second.kind === 'workbench_field') return first.kind === 'workbench_field'
    && second.kind === 'workbench_field' && sameInputTarget(
      { requestId: first.requestId, requestTitle: '', destination: first.destination },
      { requestId: second.requestId, requestTitle: '', destination: second.destination },
    )
  if (first.kind === 'question_answer' || second.kind === 'question_answer') return first.kind === 'question_answer'
    && second.kind === 'question_answer' && first.questionId === second.questionId
  if (first.kind === 'web_review_annotation' || second.kind === 'web_review_annotation') return first.kind === 'web_review_annotation'
    && second.kind === 'web_review_annotation' && first.annotationId === second.annotationId
  return first.annotationId === second.annotationId && first.field === second.field && first.sourceVersion === second.sourceVersion
}

function receiptAlreadyApplied(receipts: WriteReceipt[], receipt: WriteReceipt): boolean {
  const ids = new Set([receipt.id, ...receipt.mergedIds])
  const existing = receipts.find((item) => [item.id, ...item.mergedIds].some((id) => ids.has(id)))
  if (!existing) return false
  if (existing.id !== receipt.id || existing.requestId !== receipt.requestId || !sameReceiptDestination(existing, receipt)
    || existing.text !== receipt.text || existing.mergedIds.length !== receipt.mergedIds.length || !existing.mergedIds.every((id) => ids.has(id))) {
    throw new Error('This speech segment was already used by a different write. Your words have been preserved.')
  }
  return true
}

function recordReceipt(snapshot: FeedbackDraftSnapshot, receipts: WriteReceipt[], receipt: WriteReceipt): FeedbackDraftSnapshot {
  const envelope = decodeFeedbackDraftEnvelope(snapshot.documentJson)!
  return {
    ...snapshot,
    documentJson: JSON.stringify({ ...envelope,
      // The write and its receipt share one draft CAS, including after a lost acknowledgement.
      speechWriteback: { version: 1, operations: [...receipts, receipt] },
    }),
  }
}

/** Pure append: callers can supply the latest local draft or a freshly loaded CAS snapshot. */
export function applySpeechWriteback(workspace: SpeechWriteWorkspace, input: SpeechWriteInput): FeedbackDraftSnapshot {
  if (workspace.request.request_id !== input.requestId) throw new Error('The speech request does not match the loaded draft.')
  if (workspace.request.status === 'completed' || workspace.request.status === 'cancelled') throw new Error('This request is closed. The draft is read-only.')
  if (!input.id || typeof input.text !== 'string' || !input.text.trim() || input.text.includes('\0')) throw new Error('Speech contains no valid text.')
  if (!resolveWorkbenchDefinition(workspace.workbench)) throw new Error('This workbench is not supported. The draft is read-only.')
  const destination = input.destination
  if (destination.kind === 'unknown') throw new Error('This speech destination is not supported. Your words have been preserved.')
  if (destination.kind === 'document') return updateFeedbackDraftDocument(
    restoreFeedbackDraftSnapshot(workspace.draft.document_json, workspace.draft.body_markdown),
    (doc) => applyDraftOperation(doc, speechDocumentOperation(input)),
  )
  const envelope = decodeFeedbackDraftEnvelope(workspace.draft.document_json)
  if (!envelope) throw new Error('The draft is unavailable. Your words have been preserved.')
  const receipts = readReceipts(envelope.speechWriteback)
  // Validate the current destination before treating a retry as acknowledged.
  // A removed answer/comment must never be revived by a delayed speech write.
  const field = readWorkbenchField(workspace, input)
  const receiptBase: WriteReceiptBase = {
    id: input.id, requestId: input.requestId, text: input.text,
    mergedIds: [...new Set(input.mergedIds ?? [])].filter((id) => id !== input.id),
  }
  const receipt: WriteReceipt = destination.kind === 'workbench_field'
    ? { ...receiptBase, kind: 'workbench_field', destination: structuredClone(destination) }
    : destination.kind === 'question_answer'
      ? { ...receiptBase, kind: 'question_answer', questionId: destination.questionId }
      : destination.kind === 'web_review_annotation'
        ? { ...receiptBase, kind: 'web_review_annotation', annotationId: destination.annotationId }
        : { ...receiptBase, annotationId: destination.annotationId, field: destination.field, sourceVersion: destination.sourceVersion }
  if (receiptAlreadyApplied(receipts, receipt)) {
    return { documentJson: workspace.draft.document_json!, bodyMarkdown: workspace.draft.body_markdown }
  }
  const snapshot = recordReceipt(appendWorkbenchField(field, input.text), receipts, receipt)
  return recordFieldSpeechSegment(workspace, snapshot, input, field.value)
}

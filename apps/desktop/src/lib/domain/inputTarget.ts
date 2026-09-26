import type { DraftView, FeedbackRequestSummary, WorkbenchSpec } from '../generated/feedback'

/** An optional Action group within the feedback document. */
export type ActionIdentity = {
  actionId: string
  actionIndex: number
  title: string
}
type ActiveAction = ActionIdentity | null

/** The minimal draft context required by destination-aware writes. */
export type InputWriteWorkspace = {
  request: Pick<FeedbackRequestSummary, 'request_id' | 'status'>
  workbench?: WorkbenchSpec
  draft: Pick<DraftView, 'document_json' | 'body_markdown'>
}

export type InputDestination =
  | { kind: 'document'; action: ActiveAction }
  | { kind: 'review_annotation'; annotationId: string; field: 'body' | 'replacement'; sourceVersion: string; paragraphLabel: string }
  | { kind: 'question_answer'; questionId: string; questionLabel: string }
  /** Future targets remain recoverable without accidentally writing to the document. */
  | { kind: 'unknown'; raw: unknown }

export type InputTarget = { requestId: string; requestTitle: string; destination: InputDestination }

const record = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value)
const action = (value: unknown): value is ActiveAction => value === null || (record(value)
  && typeof value.actionId === 'string' && Number.isInteger(value.actionIndex) && typeof value.title === 'string')

export function snapshotInputTarget(target: InputTarget): InputTarget {
  return { requestId: target.requestId, requestTitle: target.requestTitle, destination: structuredClone(target.destination) }
}

/** Decode both persisted legacy action targets and current destinations. */
export function normalizeInputTarget(value: unknown): InputTarget | null {
  if (!record(value) || typeof value.requestId !== 'string' || typeof value.requestTitle !== 'string') return null
  let destination: InputDestination
  const raw = value.destination
  if (!('destination' in value) && action(value.action)) destination = { kind: 'document', action: value.action }
  else if (record(raw) && raw.kind === 'document' && action(raw.action)) destination = raw as InputDestination
  else if (record(raw) && raw.kind === 'review_annotation' && typeof raw.annotationId === 'string'
    && (raw.field === 'body' || raw.field === 'replacement') && typeof raw.sourceVersion === 'string'
    && typeof raw.paragraphLabel === 'string') destination = raw as InputDestination
  else if (record(raw) && raw.kind === 'question_answer' && typeof raw.questionId === 'string'
    && typeof raw.questionLabel === 'string') destination = raw as InputDestination
  else if (record(raw) && raw.kind === 'unknown' && 'raw' in raw) destination = { kind: 'unknown', raw: raw.raw }
  else destination = { kind: 'unknown', raw: 'destination' in value ? raw : { action: value.action } }
  return snapshotInputTarget({ requestId: value.requestId, requestTitle: value.requestTitle, destination })
}

export function sameInputTarget(first: InputTarget, second: InputTarget): boolean {
  if (first.requestId !== second.requestId) return false
  const a = first.destination
  const b = second.destination
  if (a.kind === 'document' && b.kind === 'document') return a.action?.actionId === b.action?.actionId
  if (a.kind === 'review_annotation' && b.kind === 'review_annotation') return a.annotationId === b.annotationId
    && a.field === b.field && a.sourceVersion === b.sourceVersion
  if (a.kind === 'question_answer' && b.kind === 'question_answer') return a.questionId === b.questionId
  // An unknown target has no trustworthy destination identity, so never merge it.
  return false
}

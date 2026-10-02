import { decodeFeedbackDraftEnvelope, updateFeedbackDraftState, type FeedbackDraftSnapshot } from './feedbackDraftDocument'
import type { WorkbenchSpec, WorkbenchState } from './generated/feedback'
import { decodeRegisteredWorkbenchState, resolveWorkbenchDefinition } from './workbench/definitions/registry'

export function readWorkbenchState(documentJson: string | null | undefined): WorkbenchState | null {
  return decodeRegisteredWorkbenchState(decodeFeedbackDraftEnvelope(documentJson)?.workbenchState)
}

export function withWorkbenchState(snapshot: FeedbackDraftSnapshot, state: WorkbenchState | null): FeedbackDraftSnapshot {
  return state === null ? snapshot : updateFeedbackDraftState(snapshot, state)
}

export function workbenchSubmissionIssue(spec: WorkbenchSpec | null | undefined, state: WorkbenchState | null, notes: string): 'unsupported' | 'empty' | 'incomplete' | null {
  const definition = resolveWorkbenchDefinition(spec)
  if (!definition) return 'unsupported'
  if (!notes.trim() && !(spec && definition.hasInput(spec, state))) return 'empty'
  return !spec || definition.complete(spec, state) ? null : 'incomplete'
}

export function canSubmitWorkbench(spec: WorkbenchSpec | null | undefined, state: WorkbenchState | null, notes: string): boolean {
  return workbenchSubmissionIssue(spec, state, notes) === null
}

/** User-facing validation follows the same policy as the submit button. */
export function workbenchSubmissionMessage(spec: WorkbenchSpec | null | undefined, state: WorkbenchState | null, notes: string): string | null {
  const issue = workbenchSubmissionIssue(spec, state, notes)
  if (!issue) return null
  if (issue === 'unsupported') return 'This workbench is unavailable. Open this request in a compatible client to continue.'
  const detail = spec && resolveWorkbenchDefinition(spec)?.submissionMessage?.(spec, state)
  if (detail) return detail
  return issue === 'empty' ? 'Provide workbench input or write feedback before submitting.'
    : 'Answer every question or choose an option before submitting. Notes are optional.'
}

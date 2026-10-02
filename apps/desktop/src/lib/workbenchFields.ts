import { decodeFeedbackDraftEnvelope, updateFeedbackDraftState, type FeedbackDraftSnapshot } from './feedbackDraftDocument'
import type { InputTarget, InputWriteWorkspace } from './domain/inputTarget'
import { decodeRegisteredWorkbenchState, getWorkbenchDefinition, resolveWorkbenchDefinition, workbenchDefinitions } from './workbench/definitions/registry'
import type { WorkbenchFieldAdapter } from './workbench/fields/contracts'
import { validFieldText } from './workbench/fields/value'

export { validFieldText } from './workbench/fields/value'

/** One validated field from one complete draft; replace preserves the envelope. */
export type WorkbenchField = ReturnType<typeof readWorkbenchField>

function fieldAdapterForTarget(target: InputTarget): WorkbenchFieldAdapter | undefined {
  const destination = target.destination
  const definitions = destination.kind === 'workbench_field'
    ? [getWorkbenchDefinition(destination.workbenchType, destination.version)].filter((item) => item !== undefined)
    : workbenchDefinitions
  return definitions.flatMap((definition) => definition.fields ?? []).find((adapter) => adapter.accepts(target))
}

export function sameWorkbenchFieldIdentity(captured: string, current: string, target: InputTarget): boolean {
  return captured === current || (fieldAdapterForTarget(target)?.sameIdentity?.(captured, current, target) ?? false)
}

/** Projection only: provenance reconciliation has no authority to write into a field. */
export function readWorkbenchFieldText(snapshot: FeedbackDraftSnapshot, target: InputTarget): string | null {
  const raw = decodeFeedbackDraftEnvelope(snapshot.documentJson)?.workbenchState
  const destination = target.destination
  const state = destination.kind === 'workbench_field'
    ? getWorkbenchDefinition(destination.workbenchType, destination.version)?.decodeState(raw)
    : decodeRegisteredWorkbenchState(raw)
  if (!state) return null
  try { return fieldAdapterForTarget(target)?.text(state, target) ?? null } catch { return null }
}

export function unlinkWorkbenchFieldAttachment(snapshot: FeedbackDraftSnapshot, attachmentId: string,
  removeText: (value: string) => string): FeedbackDraftSnapshot {
  const state = decodeRegisteredWorkbenchState(decodeFeedbackDraftEnvelope(snapshot.documentJson)?.workbenchState)
  if (!state) return snapshot
  const fields = getWorkbenchDefinition(state.type)?.fields ?? []
  const next = fields.reduce((current, adapter) => adapter.removeAttachment(current, attachmentId, removeText), state)
  return next === state ? snapshot : updateFeedbackDraftState(snapshot, next)
}

export function readWorkbenchField(workspace: InputWriteWorkspace, target: InputTarget) {
  if (workspace.request.request_id !== target.requestId) throw new Error('The input request is no longer open.')
  if (workspace.request.status === 'completed' || workspace.request.status === 'cancelled') throw new Error('This request is closed. The draft is read-only.')
  const definition = resolveWorkbenchDefinition(workspace.workbench)
  if (!definition) throw new Error('This workbench is not supported. The draft is read-only.')
  const envelope = decodeFeedbackDraftEnvelope(workspace.draft.document_json)
  if (!envelope) throw new Error('The draft is unavailable.')
  const state = definition.decodeState(envelope.workbenchState)
  const destination = target.destination
  if (destination.kind === 'workbench_field' && (workspace.workbench?.type !== destination.workbenchType
    || workspace.workbench.version !== destination.version)) throw new Error('This input belongs to another workbench contract.')
  const adapter = definition.fields?.find((field) => field.accepts(target))
  if (!adapter || !state || !workspace.workbench) throw new Error('This input field is unavailable.')
  const field = adapter.read({ spec: workspace.workbench, state, target })
  if (!Number.isInteger(field.limit) || field.limit < 0 || !validFieldText(field.value, field.limit)) throw new Error('This input field cannot receive text.')
  return { ...field, replace(value: string): FeedbackDraftSnapshot {
    if (!validFieldText(value, field.limit)) throw new Error('The input would exceed its text limit. Your original has been kept.')
    const next = field.replace(value)
    if (next.type !== state.type || !definition.decodeState(next)) throw new Error('The input replacement is not valid for this workbench.')
    return { documentJson: JSON.stringify({ ...envelope, workbenchState: next }), bodyMarkdown: workspace.draft.body_markdown }
  } }
}

/** Append ordinary input; provenance and idempotency belong to the caller. */
export function appendWorkbenchField(field: WorkbenchField, text: string): FeedbackDraftSnapshot {
  if (!text.trim() || text.includes('\0')) throw new Error('The input contains no valid text.')
  const value = field.value + (field.value && !/\s$/.test(field.value) ? '\n' : '') + text
  if (!validFieldText(value, field.limit)) throw new Error('The input would exceed its text limit. Your original has been kept.')
  return field.replace(value)
}

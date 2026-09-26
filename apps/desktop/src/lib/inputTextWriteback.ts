import { decodeFeedbackDraftEnvelope, type FeedbackDraftSnapshot } from './feedbackDraftDocument'
import { appendWorkbenchField, readWorkbenchField } from './workbenchFields'
import {
  normalizeInputTarget, sameInputTarget, snapshotInputTarget,
  type InputTarget, type InputWriteWorkspace,
} from './domain/inputTarget'

export type InputTextWriteInput = { target: InputTarget; text: string; id: string }
const record = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value)

function readReceipts(history: unknown): InputTextWriteInput[] {
  if (history === undefined) return []
  const invalid = () => new Error('This input history is not supported. Your current draft has been kept.')
  if (!record(history) || history.version !== 1 || !Array.isArray(history.operations)) throw invalid()
  return history.operations.map((item) => {
    if (!record(item) || typeof item.id !== 'string' || typeof item.text !== 'string') throw invalid()
    const target = normalizeInputTarget(item.target)
    if (!target || target.destination.kind === 'unknown') throw invalid()
    return { id: item.id, text: item.text, target }
  })
}

/** A receipt is saved atomically with pasted text or an attachment reference, never as speech. */
export function applyInputTextWriteback(workspace: InputWriteWorkspace, input: InputTextWriteInput): FeedbackDraftSnapshot {
  if (workspace.request.request_id !== input.target.requestId) throw new Error('The input request does not match the loaded draft.')
  if (!input.id || !input.text.trim() || input.text.includes('\0')) throw new Error('The input contains no valid text.')
  const field = readWorkbenchField(workspace, input.target)
  const envelope = decodeFeedbackDraftEnvelope(workspace.draft.document_json)
  if (!envelope) throw new Error('The draft is unavailable.')
  const receipts = readReceipts(envelope.inputWriteback)
  const previous = receipts.find((item) => item.id === input.id)
  if (previous) {
    if (previous.text !== input.text || !sameInputTarget(previous.target, input.target)) throw new Error('This input was already used by a different write.')
    return { documentJson: workspace.draft.document_json!, bodyMarkdown: workspace.draft.body_markdown }
  }
  const next = appendWorkbenchField(field, input.text)
  const updated = decodeFeedbackDraftEnvelope(next.documentJson)!
  return { ...next, documentJson: JSON.stringify({ ...updated,
    inputWriteback: { version: 1, operations: [...receipts, { ...input, target: snapshotInputTarget(input.target) }] },
  }) }
}

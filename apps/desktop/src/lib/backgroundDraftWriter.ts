import type { DraftView, FeedbackWorkspaceView, SaveDraftInput } from './feedback'
import { applyDraftOperation, type DraftOperation } from './draftOperations'
import {
  type FeedbackDraftSnapshot,
  restoreFeedbackDraftSnapshot,
  updateFeedbackDraftDocument,
} from './feedbackDraftDocument'
import { commandErrorCode } from './workbench/feedbackText'
import { applySpeechWriteback, type SpeechWriteInput } from './speech/speechWriteback'
import { applyInputTextWriteback, type InputTextWriteInput } from './inputTextWriteback'

export type BackgroundDraftWriter = {
  load: () => Promise<FeedbackWorkspaceView>
  save: (input: SaveDraftInput) => Promise<DraftView>
}

export async function writeBackgroundDraftOperation(
  requestId: string,
  operation: DraftOperation,
  writer: BackgroundDraftWriter,
  maxAttempts = 3,
): Promise<DraftView> {
  return writeBackgroundSnapshot(requestId, (workspace) => updateFeedbackDraftDocument(
    restoreFeedbackDraftSnapshot(workspace.draft.document_json, workspace.draft.body_markdown),
    (doc) => applyDraftOperation(doc, operation),
  ), writer, maxAttempts)
}

/** Speech destinations share the same compare-and-swap revision as all draft edits. */
export async function writeBackgroundSpeech(
  input: SpeechWriteInput,
  writer: BackgroundDraftWriter,
  maxAttempts = 3,
): Promise<DraftView> {
  return writeBackgroundSnapshot(input.requestId, (workspace) => applySpeechWriteback(workspace, input), writer, maxAttempts)
}

export function writeBackgroundInputText(input: InputTextWriteInput, writer: BackgroundDraftWriter, maxAttempts = 3): Promise<DraftView> {
  return writeBackgroundSnapshot(input.target.requestId, (workspace) => applyInputTextWriteback(workspace, input), writer, maxAttempts)
}

async function writeBackgroundSnapshot(requestId: string,
  apply: (workspace: FeedbackWorkspaceView) => FeedbackDraftSnapshot,
  writer: BackgroundDraftWriter, maxAttempts: number): Promise<DraftView> {
  let lastConflict: unknown
  for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
    const workspace = await writer.load()
    if (workspace.request.request_id !== requestId) {
      throw new Error('loaded workspace ' + workspace.request.request_id + ' for ' + requestId)
    }
    const next = apply(workspace)
    if (workspace.draft.document_json === next.documentJson && workspace.draft.body_markdown === next.bodyMarkdown) return workspace.draft
    try {
      return await writer.save({ request_id: requestId, document_json: next.documentJson,
        body_markdown: next.bodyMarkdown, expected_revision: workspace.draft.saved_revision })
    } catch (cause) {
      if (commandErrorCode(cause) !== 'DRAFT_CONFLICT' || attempt + 1 >= maxAttempts) throw cause
      lastConflict = cause
    }
  }
  throw lastConflict ?? new Error('background draft write did not run')
}

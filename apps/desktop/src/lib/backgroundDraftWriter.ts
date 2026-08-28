import type { DraftView, FeedbackWorkspaceView, SaveDraftInput } from './feedback'
import { applyDraftOperation, type DraftOperation } from './draftOperations'
import {
  restoreFeedbackDraftDocument,
  snapshotFeedbackDraftDocument,
} from './feedbackDraftDocument'
import { commandErrorCode } from './workbench/feedbackText'

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
  let lastConflict: unknown
  for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
    const workspace = await writer.load()
    if (workspace.request.request_id !== requestId) {
      throw new Error(`loaded workspace ${workspace.request.request_id} for ${requestId}`)
    }
    const next = snapshotFeedbackDraftDocument(
      applyDraftOperation(
        restoreFeedbackDraftDocument(
          workspace.draft.document_json,
          workspace.draft.body_markdown,
        ),
        operation,
      ),
    )
    if (
      workspace.draft.document_json === next.documentJson &&
      workspace.draft.body_markdown === next.bodyMarkdown
    ) {
      return workspace.draft
    }
    try {
      return await writer.save({
        request_id: requestId,
        document_json: next.documentJson,
        body_markdown: next.bodyMarkdown,
        expected_revision: workspace.draft.saved_revision,
      })
    } catch (cause) {
      if (commandErrorCode(cause) !== 'DRAFT_CONFLICT' || attempt + 1 >= maxAttempts) {
        throw cause
      }
      lastConflict = cause
    }
  }
  throw lastConflict ?? new Error('background draft write did not run')
}

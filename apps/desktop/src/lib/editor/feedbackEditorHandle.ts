import type { DraftOperation } from '../draftOperations'
import type { SpeechCleanupSegment } from '../speech/speechBlockMetadata'

/** Contract the feedback editor exposes to the controllers that write into it. */
export type FeedbackEditorHandle = {
  removeAttachmentReference(attachmentId: string): void
  applyDraftOperation(operation: DraftOperation): boolean
  pendingSpeechSegments(): SpeechCleanupSegment[]
  replaceSpeechSegments(
    replacements: Array<{ segmentId: string; originalText: string; nextText: string }>,
  ): boolean
}

import type { FeedbackDraftSnapshot } from '../../feedbackDraftDocument'
import { removeFieldAttachment } from '../../input/fieldAttachmentText'
import { unlinkWorkbenchFieldAttachment } from '../../workbenchFields'

/** Unlink a deleted attachment from the structured workbench draft as well as its text fields. */
export function removeWorkbenchAttachmentReferences(snapshot: FeedbackDraftSnapshot, attachmentId: string): FeedbackDraftSnapshot {
  return unlinkWorkbenchFieldAttachment(snapshot, attachmentId, (value) =>
    removeFieldAttachment(value, attachmentId, [{ attachment_id: attachmentId }]))
}

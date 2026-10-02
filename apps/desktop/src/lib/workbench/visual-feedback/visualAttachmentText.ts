import type { AttachmentView } from '../../feedback'
import { fieldAttachmentText } from '../../input/fieldAttachmentText'

/** Match field text without changing the structured source or hiding unknown URLs/code. */
export function visualAttachmentText(value: string, attachments: readonly Pick<AttachmentView, 'attachment_id'>[] = []): string {
  return fieldAttachmentText(value, attachments).text
}

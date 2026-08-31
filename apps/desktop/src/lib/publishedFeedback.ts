import { attachmentMarkdownUrl } from './attachmentMarkdown'
import type { FeedbackPackageAttachment, FeedbackPackageView } from './generated/feedback'
import { operatorFeedbackBody } from './workbench/feedbackText'

export type PublishedFeedbackView = {
  markdown: string
  uncooked_markdown?: string
}

export type PublishedFeedbackPackage = FeedbackPackageView
export type PublishedAttachmentPath = Pick<FeedbackPackageAttachment, 'id' | 'path'>

/**
 * Normalize a published feedback package into the workbench view: extract the
 * Operator Feedback body and rewrite portable attachment paths to local
 * `attachment://id` URLs. Display-only; never changes the package on disk.
 */
export function normalizePublishedFeedback(
  published: PublishedFeedbackPackage | null,
): PublishedFeedbackView | null {
  if (!published) return null
  const attachments = published.manifest?.attachments ?? []
  return {
    markdown: restorePublishedAttachmentUrls(
      operatorFeedbackBody(published.markdown),
      attachments,
    ),
    uncooked_markdown: published.uncooked_markdown
      ? restorePublishedAttachmentUrls(
          operatorFeedbackBody(published.uncooked_markdown),
          attachments,
        )
      : undefined,
  }
}

/**
 * The immutable package uses portable `attachments/...` paths. The workbench
 * uses its local `attachment://id` scheme so it can safely hydrate previews.
 * This is display-only and never changes the published package on disk.
 */
/** True when a closed request actually has a cooked body distinct from the original. */
export function hasCookedPublishedVariant(
  cookedMarkdown: string,
  uncookedMarkdown: string | undefined,
): boolean {
  const cooked = cookedMarkdown.trim()
  const uncooked = uncookedMarkdown?.trim() ?? ''
  return cooked.length > 0 && uncooked.length > 0 && cooked !== uncooked
}

export function restorePublishedAttachmentUrls(
  markdown: string,
  attachments: PublishedAttachmentPath[],
): string {
  return attachments.reduce((output, attachment) => {
    const path = attachment.path.replaceAll('\\', '/')
    if (!path) return output
    const escaped = path.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    return output.replace(
      new RegExp(`\\]\\(${escaped}(?=\\s|\\))`, 'g'),
      `](${attachmentMarkdownUrl(attachment.id)}`,
    )
  }, markdown)
}

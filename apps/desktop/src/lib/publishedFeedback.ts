import { attachmentMarkdownUrl } from './attachmentMarkdown'

export type PublishedAttachmentPath = {
  id: string
  path: string
}

/**
 * The immutable package uses portable `attachments/...` paths. The workbench
 * uses its local `attachment://id` scheme so it can safely hydrate previews.
 * This is display-only and never changes the published package on disk.
 */
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

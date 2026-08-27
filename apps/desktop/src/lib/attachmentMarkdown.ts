export const ATTACHMENT_SCHEME = 'attachment://'

/** 1px transparent GIF: keeps layout without loading an unsupported scheme. */
export const ATTACHMENT_PLACEHOLDER_IMAGE =
  'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw=='

export function attachmentMarkdownUrl(attachmentId: string): string {
  return `${ATTACHMENT_SCHEME}${attachmentId}`
}

export function attachmentIdFromUrl(value: unknown): string | null {
  if (typeof value !== 'string' || !value.startsWith(ATTACHMENT_SCHEME)) return null
  const attachmentId = value.slice(ATTACHMENT_SCHEME.length)
  return attachmentId.length > 0 ? attachmentId : null
}

export function isImageMediaType(mediaType: string): boolean {
  return mediaType.startsWith('image/')
}

export function attachmentMarkdown(attachment: {
  attachment_id: string
  file_name: string
  media_type: string
}): string {
  const url = attachmentMarkdownUrl(attachment.attachment_id)
  const name = attachment.file_name.replace(/([\[\]])/g, '\\$1')
  return isImageMediaType(attachment.media_type)
    ? `![${name}](${url})`
    : `[${name}](${url})`
}

export const ATTACHMENT_SCHEME = 'attachment://'

export function attachmentMarkdownUrl(attachmentId: string): string {
  return `${ATTACHMENT_SCHEME}${attachmentId}`
}

export function attachmentIdFromUrl(value: unknown): string | null {
  if (typeof value !== 'string' || !value.startsWith(ATTACHMENT_SCHEME)) return null
  const attachmentId = value.slice(ATTACHMENT_SCHEME.length)
  return attachmentId.length > 0 ? attachmentId : null
}

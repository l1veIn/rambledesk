import type { FeedbackPackageView } from './generated/feedback'

export type PublishedFeedbackDownload = Readonly<{
  fileName: string
  mediaType: 'application/json'
  contents: string
}>

/** Browser-safe export of the authenticated published projection, not a server path or ZIP. */
export function publishedFeedbackDownload(
  requestId: string,
  published: FeedbackPackageView,
): PublishedFeedbackDownload {
  const safeRequestId = requestId.replaceAll(/[^A-Za-z0-9_-]/gu, '_')
  return {
    fileName: `${safeRequestId || 'feedback'}.rambledesk-feedback.json`,
    mediaType: 'application/json',
    contents: `${JSON.stringify(published, null, 2)}\n`,
  }
}

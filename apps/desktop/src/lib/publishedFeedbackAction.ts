import type { ApplicationTransport } from './application/applicationTransport'
import { publishedFeedbackDownload } from './publishedFeedbackDownload'

export type PublishedFeedbackAction = Readonly<{
  label: 'Open feedback package' | 'Download published feedback'
  run: (requestId: string) => Promise<void>
}>

export function createBrowserPublishedFeedbackAction(
  transport: ApplicationTransport,
): PublishedFeedbackAction {
  return {
    label: 'Download published feedback',
    async run(requestId) {
      const published = await transport.call('readPublishedFeedback', { request_id: requestId })
      if (!published) throw new Error('Published feedback is unavailable.')
      const download = publishedFeedbackDownload(requestId, published)
      const url = URL.createObjectURL(
        new Blob([download.contents], { type: download.mediaType }),
      )
      try {
        const anchor = document.createElement('a')
        anchor.href = url
        anchor.download = download.fileName
        anchor.click()
      } finally {
        URL.revokeObjectURL(url)
      }
    },
  }
}

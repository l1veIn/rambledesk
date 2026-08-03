import { describe, expect, it } from 'vitest'

import { restorePublishedAttachmentUrls } from './publishedFeedback'

describe('restorePublishedAttachmentUrls', () => {
  it('maps package-relative Markdown destinations to local attachment URLs', () => {
    const markdown = [
      '![capture](attachments/001-capture.png)',
      '[open attachment](attachments/001-capture.png "capture")',
      'The text attachments/001-capture.png is left alone.',
    ].join('\n')

    expect(
      restorePublishedAttachmentUrls(markdown, [
        { id: 'attachment-id', path: 'attachments/001-capture.png' },
      ]),
    ).toBe([
      '![capture](attachment://attachment-id)',
      '[open attachment](attachment://attachment-id "capture")',
      'The text attachments/001-capture.png is left alone.',
    ].join('\n'))
  })

  it('preserves attachment URLs that are already local', () => {
    expect(
      restorePublishedAttachmentUrls('![capture](attachment://already-local)', [
        { id: 'attachment-id', path: 'attachments/001-capture.png' },
      ]),
    ).toBe('![capture](attachment://already-local)')
  })
})

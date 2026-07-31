import { describe, expect, it } from 'vitest'

import {
  attachmentIdFromUrl,
  attachmentMarkdownUrl,
} from './attachmentMarkdown'

describe('attachment Markdown URLs', () => {
  it('round-trips an attachment id without touching Chinese alt text', () => {
    const attachmentId = '6cb2a15c-e493-43d2-9674-a76120c33f38'
    const markdown = `![中文截图](${attachmentMarkdownUrl(attachmentId)})`

    expect(markdown).toBe(
      '![中文截图](attachment://6cb2a15c-e493-43d2-9674-a76120c33f38)',
    )
    expect(
      attachmentIdFromUrl(markdown.slice(markdown.indexOf('(') + 1, -1)),
    ).toBe(attachmentId)
  })

  it('rejects normal and empty URLs', () => {
    expect(attachmentIdFromUrl('attachments/001-image.png')).toBeNull()
    expect(attachmentIdFromUrl('attachment://')).toBeNull()
  })
})

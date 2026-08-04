import { describe, expect, it } from 'vitest'

import {
  attachmentIdFromUrl,
  attachmentMarkdown,
  attachmentMarkdownUrl,
  isImageMediaType,
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

describe('attachmentMarkdown', () => {
  it('renders image attachments as image markdown', () => {
    expect(
      attachmentMarkdown({
        attachment_id: 'id-1',
        file_name: 'a.png',
        media_type: 'image/png',
      }),
    ).toBe('![a.png](attachment://id-1)')
  })

  it('renders non-image attachments as link markdown', () => {
    expect(
      attachmentMarkdown({
        attachment_id: 'id-2',
        file_name: 'plan.pdf',
        media_type: 'application/pdf',
      }),
    ).toBe('[plan.pdf](attachment://id-2)')
  })

  it('escapes brackets in the link label', () => {
    expect(
      attachmentMarkdown({
        attachment_id: 'id-3',
        file_name: 'a[b].md',
        media_type: 'text/markdown',
      }),
    ).toBe('[a\\[b\\].md](attachment://id-3)')
  })

  it('classifies image media types', () => {
    expect(isImageMediaType('image/png')).toBe(true)
    expect(isImageMediaType('image/webp')).toBe(true)
    expect(isImageMediaType('application/pdf')).toBe(false)
    expect(isImageMediaType('text/markdown')).toBe(false)
  })
})

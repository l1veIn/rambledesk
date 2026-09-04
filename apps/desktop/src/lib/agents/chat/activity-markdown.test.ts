import { describe, expect, it } from 'vitest'
import { getSchema } from '@tiptap/core'
import { feedbackEditorExtensions } from '$lib/feedbackEditorExtensions'
import { activityMarkdownDocument, sanitizeActivityDocument } from './activity-markdown'

describe('agent Markdown documents', () => {
  it('reuses the real editor schema for headings, fenced code, lists and tables', () => {
    const document = activityMarkdownDocument('# Result\n\n```ts\nconst answer = 42\n```\n\n- One\n- Two\n\n| A | B |\n| - | - |\n| 1 | 2 |')
    const types = JSON.stringify(document)
    expect(types).toContain('heading')
    expect(types).toContain('codeBlock')
    expect(types).toContain('table')
    const node = getSchema(feedbackEditorExtensions()).nodeFromJSON(document)
    expect(() => node.check()).not.toThrow()
    expect(node.textContent).toContain('const answer = 42')
  })

  it('turns unsolicited media and attachment actions into inert schema-valid text while keeping safe HTTP links', () => {
    const document = sanitizeActivityDocument({ type: 'doc', content: [
      { type: 'image', attrs: { src: 'https://example.com/tracker.png', alt: 'remote image' } },
      { type: 'paragraph', content: [
        { type: 'text', text: 'Unsafe', marks: [{ type: 'link', attrs: { href: 'javascript:alert(1)' } }] },
        { type: 'text', text: 'Docs', marks: [{ type: 'link', attrs: { href: 'https://example.com/docs', attachmentId: 'fake' } }] },
      ] },
      { type: 'attachmentFile', attrs: { fileName: 'source.ts', attachmentId: 'fake' } },
    ] })
    const json = JSON.stringify(document)
    expect(json).not.toContain('javascript:')
    expect(json).not.toContain('tracker.png')
    expect(json).not.toContain('attachmentId')
    expect(json).toContain('https://example.com/docs')
    expect(json).toContain('[source.ts]')
    expect(() => getSchema(feedbackEditorExtensions()).nodeFromJSON(document).check()).not.toThrow()
  })
})

// @vitest-environment jsdom
import { Editor } from '@tiptap/core'
import { MarkdownManager } from '@tiptap/markdown'
import { describe, expect, it } from 'vitest'

import {
  feedbackEditorExtensions,
  parseFeedbackMarkdown,
  serializeFeedbackMarkdown,
} from './feedbackEditorExtensions'

// This is the actual default instance used by TipTap when no instance is
// supplied. No mock: leaked tokenizer closures remain registered here even
// after the Editor that created them has been destroyed.
const sharedMarked = new MarkdownManager({ extensions: [] }).instance

function tokenizers(parser = sharedMarked) {
  return {
    block: parser.defaults.extensions?.block?.length ?? 0,
    inline: parser.defaults.extensions?.inline?.length ?? 0,
    startBlock: parser.defaults.extensions?.startBlock?.length ?? 0,
    startInline: parser.defaults.extensions?.startInline?.length ?? 0,
  }
}

describe('feedback markdown parser ownership', () => {
  it('does not retain parser registrations across repeated conversions', () => {
    const before = tokenizers()
    for (let index = 0; index < 20; index += 1) {
      const document = parseFeedbackMarkdown('[notes.pdf](attachment://abc-123)')
      expect(serializeFeedbackMarkdown(document)).toBe('[notes.pdf](attachment://abc-123)')
    }
    expect(tokenizers()).toEqual(before)
  })

  it('does not retain parser registrations after real editor destruction', () => {
    const before = tokenizers()
    const anchor = new Editor({ extensions: feedbackEditorExtensions() })
    const anchorParser = anchor.markdown!.instance
    const anchorBefore = tokenizers(anchorParser)
    try {
      for (let index = 0; index < 20; index += 1) {
        const editor = new Editor({
          element: document.createElement('div'),
          extensions: feedbackEditorExtensions(),
          content: '[notes.pdf](attachment://abc-123)',
          contentType: 'markdown',
        })
        try {
          expect(editor.markdown!.instance).not.toBe(anchorParser)
          expect(editor.getJSON().content?.[0]?.content?.[0]?.type).toBe('attachmentFile')
          expect(editor.getMarkdown()).toBe('[notes.pdf](attachment://abc-123)')
        } finally {
          editor.destroy()
        }
      }
      expect(tokenizers(anchorParser)).toEqual(anchorBefore)
    } finally {
      anchor.destroy()
    }
    expect(tokenizers()).toEqual(before)
  })

  it('keeps free-form hard breaks local to the requested conversion', () => {
    const source = 'first line\nsecond line'
    const canonical = parseFeedbackMarkdown(source)
    expect(JSON.stringify(canonical)).not.toContain('hardBreak')

    expect(JSON.stringify(parseFeedbackMarkdown(source, { breaks: true }))).toContain('hardBreak')

    expect(parseFeedbackMarkdown(source)).toEqual(canonical)
    const editor = new Editor({
      element: document.createElement('div'),
      extensions: feedbackEditorExtensions(),
      content: source,
      contentType: 'markdown',
    })
    try {
      expect(editor.getJSON()).toMatchObject(canonical)
    } finally {
      editor.destroy()
    }
  })
})

// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { Editor } from '@tiptap/core'
import { fieldDocumentText, fieldTextDocument, fieldTextExtensions, fieldTextPosition } from './fieldTextDocument'

const editors: Editor[] = []
function input(text: string) {
  const editor = new Editor({ extensions: fieldTextExtensions(), content: fieldTextDocument(text) })
  editors.push(editor)
  return editor
}
afterEach(() => { editors.splice(0).forEach((editor) => editor.destroy()) })

describe('plain field TipTap document', () => {
  it.each(['', '\n', '\n\n', '  spaces  \n\nlast\n', '第一行😀\n\n结尾👨‍👩‍👧', '# Heading\n**literal** [link](https://example.com)\n- item', 'Keep\r\nCRLF'])
    ('round-trips field text without Markdown parsing: %j', (value) => {
      const editor = input(value)
      expect(fieldDocumentText(editor.getJSON())).toBe(value)
      expect(editor.getJSON().content?.every((node) => node.type === 'paragraph')).toBe(true)
    })

  it('maps Unicode scalars, empty paragraphs, and hard breaks to real editor positions', () => {
    const editor = input('😀A\n\n文😀')
    expect([0, 1, 2, 3, 4, 5, 6].map((offset) => fieldTextPosition(editor.state.doc, offset)))
      .toEqual([1, 3, 4, 6, 8, 9, 11])
    editor.commands.setContent({ type: 'doc', content: [{ type: 'paragraph', content: [
      { type: 'text', text: '😀' }, { type: 'hardBreak' }, { type: 'text', text: '文' },
    ] }] })
    expect(fieldDocumentText(editor.getJSON())).toBe('😀\n文')
    expect([0, 1, 2, 3].map((offset) => fieldTextPosition(editor.state.doc, offset))).toEqual([1, 3, 4, 5])
    expect(fieldTextPosition(editor.state.doc, -1)).toBe(1)
    expect(fieldTextPosition(editor.state.doc, 100)).toBe(5)
  })

  it('keeps the shared editor undo and newline commands while excluding rich formatting', () => {
    const editor = input('first')
    editor.commands.setTextSelection(6)
    editor.commands.splitBlock()
    editor.commands.insertContent('second')
    expect(fieldDocumentText(editor.getJSON())).toBe('first\nsecond')
    expect(editor.commands.undo()).toBe(true)
    expect(fieldDocumentText(editor.getJSON())).toBe('first')
    expect(editor.schema.marks).toEqual({})
    expect(Object.keys(editor.schema.nodes)).toEqual(expect.arrayContaining(['doc', 'paragraph', 'text', 'hardBreak']))
    expect(editor.schema.nodes.heading).toBeUndefined()
  })
})

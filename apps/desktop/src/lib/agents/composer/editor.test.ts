// Adapted from Codeg composer editor-config / plain-text-content / quote-insert tests at 3ebdfed.
// SPDX-License-Identifier: Apache-2.0
// RambleDesk: real headless Tiptap Editor instances, without a DOM or React harness.
import { Editor } from '@tiptap/core'
import { afterEach, describe, expect, it } from 'vitest'
import { buildComposerExtensions } from './editor-config'
import { replaceComposerText } from './composer-commands'
import { decidePastedContent, textToInlineContent, textToSeededDoc } from './plain-text-content'
import { quoteLineDecorations } from './quote-decoration'
import { composerLeafText, serializeDocToText } from './to-prompt-blocks'

const editors: Editor[] = []
function makeEditor(text = '') {
  const editor = new Editor({ element: null, content: textToSeededDoc(text), extensions: buildComposerExtensions() })
  editor.view.updateState(editor.state.reconfigure({ plugins: editor.extensionManager.plugins }))
  editors.push(editor)
  return editor
}
afterEach(() => { for (const editor of editors.splice(0)) editor.destroy() })

describe('Codeg plain-text composer port', () => {
  it('registers no Markdown parser, formatting node, mark, or formatting input rule', () => {
    const editor = makeEditor()
    const names = editor.extensionManager.extensions.map((extension) => extension.name)
    for (const name of ['markdown', 'link', 'bold', 'italic', 'strike', 'code', 'codeBlock', 'heading', 'blockquote', 'bulletList', 'orderedList']) expect(names).not.toContain(name)
    expect(Object.keys(editor.schema.marks)).toEqual([])
  })

  it.each(['# Heading', '**bold** and _italic_', '- [ ] checklist', '```ts\nconst literal = true\n```', '[link](https://example.com)', '<script>alert(1)</script>', '  首行\n\n第二行 😀\n'])('round-trips literal draft text: %s', (text) => {
    const editor = makeEditor(text)
    expect(serializeDocToText(editor.state.doc)).toBe(text)
    expect(editor.getJSON().content?.every((node) => node.type === 'paragraph')).toBe(true)
    expect(serializeDocToText(editor.schema.nodeFromJSON(editor.getJSON()))).toBe(text)
  })

  it('uses actual editor transactions for multi-line text insertion and hard breaks', () => {
    const editor = makeEditor()
    editor.commands.insertContent(textToInlineContent('one\n\n# literal'))
    editor.commands.setHardBreak()
    editor.commands.insertContent(textToInlineContent('**two**'))
    expect(serializeDocToText(editor.state.doc)).toBe('one\n\n# literal\n**two**')
    expect(editor.state.doc.firstChild?.content.childCount).toBe(6)
  })

  it('prefers plain clipboard URLs over rich HTML titles and preserves native PM slices', () => {
    const editor = makeEditor()
    const paste = decidePastedContent({ html: '<a href="https://example.com">A title</a>', text: 'https://example.com\nnext' })
    expect(paste).not.toBeNull()
    editor.commands.insertContent(paste!)
    expect(serializeDocToText(editor.state.doc)).toBe('https://example.com\nnext')
    expect(decidePastedContent({ html: '<p data-pm-slice="1 1 []">one<br>two</p>', text: 'onetwo' })).toBeNull()
    const slice = editor.state.doc.slice(1, editor.state.doc.content.size - 1)
    expect(slice.content.textBetween(0, slice.content.size, '\n', composerLeafText)).toBe('https://example.com\nnext')
  })

  it('keeps nested quote markers and blank quote lines in send text while decorating their positions', () => {
    const text = 'What does this mean?\n\n> first\n>\n> > nested\n\n'
    const editor = makeEditor(text)
    expect(serializeDocToText(editor.state.doc)).toBe(text)
    expect(quoteLineDecorations(editor.state.doc).find()).toHaveLength(4)
  })

  it('serializes a real file reference atom in place without losing following quote positions', () => {
    const editor = makeEditor()
    editor.commands.insertContent([
      { type: 'reference', attrs: { id: 'source', label: 'a [1].ts', uri: 'file:///repo/a (1).ts' } },
      { type: 'hardBreak' }, { type: 'text', text: '> inspect this' },
    ])
    expect(serializeDocToText(editor.state.doc)).toBe('[a \\[1\\].ts](<file:///repo/a (1).ts>)\n> inspect this')
    expect(quoteLineDecorations(editor.state.doc).find().map(({ from, to }) => [from, to])).toEqual([[3, 5]])
  })

  it('resets undo when switching controlled drafts, so another session cannot be restored with Undo', () => {
    const editor = makeEditor('Session one')
    editor.commands.setTextSelection(editor.state.doc.content.size - 1)
    editor.commands.insertContent(textToInlineContent(' private draft'))
    expect(editor.commands.undo()).toBe(true)
    editor.commands.insertContent(textToInlineContent(' previous session'))
    replaceComposerText(editor, 'Session two', true)
    expect(editor.commands.undo()).toBe(false)
    expect(serializeDocToText(editor.state.doc)).toBe('Session two')
  })

  it('does not notify controlled draft changes when applying external content', () => {
    const editor = makeEditor()
    const updates: string[] = []
    editor.on('update', () => updates.push(serializeDocToText(editor.state.doc)))
    replaceComposerText(editor, 'Loaded draft')
    expect(updates).toEqual([])
    editor.commands.setTextSelection(editor.state.doc.content.size - 1)
    editor.commands.insertContent(textToInlineContent(' typing'))
    expect(updates).toEqual(['Loaded draft typing'])
  })
})

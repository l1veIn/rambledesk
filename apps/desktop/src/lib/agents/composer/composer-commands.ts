// Adapted from Codeg composer/composer-commands.ts and quote-insert.test.ts at 3ebdfed.
// SPDX-License-Identifier: Apache-2.0
// RambleDesk: controlled text drafts and session-isolated undo history.
import type { Editor } from '@tiptap/core'
import { EditorState } from '@tiptap/pm/state'
import { buildQuotedMarkdown } from './message-quote'
import { textToInlineContent, textToSeededDoc } from './plain-text-content'
import { serializeDocToText } from './to-prompt-blocks'

export function replaceComposerText(editor: Editor, text: string, resetHistory = false): void {
  editor.commands.setContent(textToSeededDoc(text), { emitUpdate: false })
  if (resetHistory) {
    editor.view.updateState(EditorState.create({ schema: editor.state.schema, doc: editor.state.doc, plugins: editor.state.plugins }))
  }
}

export function insertComposerText(editor: Editor, text: string): boolean {
  return text.length > 0 && editor.commands.insertContent(textToInlineContent(text))
}

export function appendComposerQuote(editor: Editor, text: string): boolean {
  const quote = buildQuotedMarkdown(text)
  if (!quote) return false
  const current = serializeDocToText(editor.state.doc)
  const gap = !current || current.endsWith('\n\n') ? '' : current.endsWith('\n') ? '\n' : '\n\n'
  return editor.commands.insertContentAt(editor.state.doc.content.size - 1, textToInlineContent(`${gap}${quote}\n\n`))
}

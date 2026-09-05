// Adapted from Codeg composer/composer-commands.ts and quote-insert.test.ts at 3ebdfed.
// SPDX-License-Identifier: Apache-2.0
// RambleDesk: controlled text drafts and session-isolated undo history.
import type { Editor } from '@tiptap/core'
import { EditorState } from '@tiptap/pm/state'
import { textToSeededDoc } from './plain-text-content'

export function replaceComposerText(editor: Editor, text: string, resetHistory = false): void {
  editor.commands.setContent(textToSeededDoc(text), { emitUpdate: false })
  if (resetHistory) {
    editor.view.updateState(EditorState.create({ schema: editor.state.schema, doc: editor.state.doc, plugins: editor.state.plugins }))
  }
}

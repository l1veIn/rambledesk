// Adapted from Codeg src/components/chat/composer/to-prompt-blocks.ts at 3ebdfed.
// SPDX-License-Identifier: Apache-2.0
// RambleDesk: the current ACP input contract accepts text; no synthetic attachment links are omitted.
import type { Editor } from '@tiptap/core'
import type { Node as ProseMirrorNode } from '@tiptap/pm/model'
import { referenceToMarkdown } from './reference-node'
import type { ComposerPromptBlock, ComposerReference } from './types'

export function composerLeafText(leaf: ProseMirrorNode): string {
  if (leaf.type.name === 'reference') return referenceToMarkdown(leaf.attrs as ComposerReference)
  return leaf.type.name === 'hardBreak' ? '\n' : ''
}

export function serializeDocToText(doc: ProseMirrorNode): string {
  return doc.textBetween(0, doc.content.size, '\n', composerLeafText)
}

export function docToPromptBlocks(editor: Editor): ComposerPromptBlock[] {
  const text = serializeDocToText(editor.state.doc).trim()
  return text ? [{ type: 'text', text }] : []
}

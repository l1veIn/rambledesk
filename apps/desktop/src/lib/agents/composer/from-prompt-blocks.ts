// Adapted from Codeg src/components/chat/composer/from-prompt-blocks.ts at 3ebdfed.
// SPDX-License-Identifier: Apache-2.0
// RambleDesk: inverse for supported text blocks; non-text blocks require the host's future attachment adapter.
import type { JSONContent } from '@tiptap/core'
import { textToSeededDoc } from './plain-text-content'
import type { ComposerPromptBlock } from './types'

export function blocksToRestoredDraft(blocks: readonly ComposerPromptBlock[]): { text: string; document: JSONContent } {
  const text = blocks.map((block) => {
    if (block.type !== 'text') throw new Error('This composer requires a host adapter for non-text content.')
    return block.text
  }).join('\n')
  return { text, document: textToSeededDoc(text) }
}

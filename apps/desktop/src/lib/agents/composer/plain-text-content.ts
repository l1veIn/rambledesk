// Adapted from Codeg src/components/chat/composer/plain-text-content.ts at 3ebdfed.
// SPDX-License-Identifier: Apache-2.0
// RambleDesk: text-only drafts; references hydrate only through a real provider.
import type { JSONContent } from '@tiptap/core'

export function textToInlineContent(text: string): JSONContent[] {
  if (!text) return []
  const out: JSONContent[] = []
  text.replace(/\r\n?/g, '\n').split('\n').forEach((line, index) => {
    if (index > 0) out.push({ type: 'hardBreak' })
    if (line.length > 0) out.push({ type: 'text', text: line })
  })
  return out
}

export function textToSeededDoc(text: string): JSONContent {
  return { type: 'doc', content: [{ type: 'paragraph', content: textToInlineContent(text) }] }
}

export type ClipboardTextSnapshot = Readonly<{ html: string; text: string }>

export function decidePastedContent(snapshot: ClipboardTextSnapshot): JSONContent[] | null {
  // Preserve native ProseMirror copies (hard breaks and reference atoms).
  if (snapshot.html.includes('data-pm-slice') || snapshot.html.includes('data-ramble-reference')) return null
  // External rich paste must prefer the actual plain-text URL over its HTML title.
  return snapshot.text ? textToInlineContent(snapshot.text) : null
}

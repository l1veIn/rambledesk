import type { AnyExtension, JSONContent } from '@tiptap/core'
import type { Node as ProseMirrorNode } from '@tiptap/pm/model'
import StarterKit from '@tiptap/starter-kit'

/** Structured fields remain plain strings. Markdown-looking input is literal text. */
export function fieldTextDocument(value: string): JSONContent {
  return { type: 'doc', content: value.split('\n').map((text) => ({ type: 'paragraph',
    ...(text ? { content: [{ type: 'text', text }] } : {}),
  })) }
}

function inlineText(node: JSONContent): string {
  if (node.type === 'hardBreak') return '\n'
  return node.text ?? (node.content ?? []).map(inlineText).join('')
}

/** Preserve empty lines and trailing newlines, including Shift+Enter hard breaks. */
export function fieldDocumentText(document: JSONContent): string {
  return (document.content ?? []).map(inlineText).join('\n')
}

/** Translate stored Unicode scalar offsets to ProseMirror's UTF-16 positions. */
export function fieldTextPosition(document: ProseMirrorNode, scalarOffset: number): number {
  let remaining = Math.max(0, Math.trunc(scalarOffset))
  let result = Math.max(1, document.content.size - 1)
  let found = false
  document.forEach((block, blockOffset, index) => {
    if (found) return
    if (index > 0) remaining -= 1 // A paragraph boundary contributes one newline.
    const start = blockOffset + 1
    if (remaining <= 0) { result = start; found = true; return }
    block.descendants((node, offset) => {
      if (found) return false
      const text = node.isText ? node.text! : node.type.name === 'hardBreak' ? '\n' : ''
      if (!text) return
      const characters = [...text]
      if (remaining <= characters.length) {
        result = start + offset + characters.slice(0, remaining).join('').length
        found = true
        return false
      }
      remaining -= characters.length
    })
    if (!found && remaining === 0) { result = blockOffset + block.nodeSize - 1; found = true }
  })
  return result
}

/** The same TipTap editing engine, with only the nodes representable by a field string. */
export function fieldTextExtensions(): AnyExtension[] {
  return [StarterKit.configure({
    blockquote: false, bold: false, bulletList: false, code: false, codeBlock: false,
    heading: false, horizontalRule: false, italic: false, listItem: false,
    listKeymap: false, link: false, orderedList: false, strike: false, underline: false,
    trailingNode: false,
  })]
}

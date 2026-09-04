import type { JSONContent } from '@tiptap/core'
import { parseFeedbackMarkdown } from '$lib/feedbackEditorExtensions'
import { isSafeHttpUrl } from '$lib/linkify'

/** Reuse the existing schema renderer while keeping agent-authored media/attachments inert. */
export function activityMarkdownDocument(markdown: string): JSONContent {
  return sanitizeActivityDocument(parseFeedbackMarkdown(markdown))
}

export function sanitizeActivityDocument(node: JSONContent, parent = 'doc'): JSONContent {
  if (node.type === 'image' || node.type === 'attachmentFile') {
    const label = String(node.attrs?.alt || node.attrs?.fileName || node.attrs?.src || 'Image')
    const text = { type: 'text', text: `[${label}]` }
    return ['paragraph', 'heading', 'codeBlock'].includes(parent) ? text : { type: 'paragraph', content: [text] }
  }
  const marks = node.marks?.filter((mark) => mark.type !== 'link' || isSafeHttpUrl(String(mark.attrs?.href ?? '')))
    .map((mark) => mark.type === 'link' ? { type: 'link', attrs: { href: mark.attrs?.href, target: '_blank', rel: 'noopener noreferrer' } } : mark)
  return { ...node, ...(marks ? { marks } : {}), ...(node.content ? { content: node.content.map((child) => sanitizeActivityDocument(child, node.type)) } : {}) }
}

import { Marked, type Token } from 'marked'
import type { AttachmentView } from '../feedback'
import { attachmentIdFromUrl } from '../attachmentMarkdown'
import type { FeedbackDraftSnapshot } from '../feedbackDraftDocument'
import { unlinkWorkbenchFieldAttachment } from '../workbenchFields'

type KnownAttachment = Pick<AttachmentView, 'attachment_id'>
type Reference = { attachmentId: string; from: number; to: number; raw: string }
const markdown = new Marked()

/** Read only real Markdown links/images, never code or unknown attachment URLs. */
function references(value: string, attachments: readonly KnownAttachment[]): Reference[] {
  const known = new Set(attachments.map((attachment) => attachment.attachment_id))
  const found: Reference[] = []
  function visit(tokens: Token[], source: string, offset: number) {
    let cursor = 0
    for (const token of tokens) {
      const start = source.indexOf(token.raw, cursor)
      if (start < 0 || !token.raw) continue
      cursor = start + token.raw.length
      const absolute = offset + start
      if (token.type === 'link' || token.type === 'image') {
        const attachmentId = attachmentIdFromUrl(token.href)
        if (attachmentId && known.has(attachmentId)) {
          found.push({ attachmentId, from: absolute, to: absolute + token.raw.length, raw: token.raw })
          continue
        }
      }
      if (token.type === 'code' || token.type === 'codespan' || token.type === 'html') continue
      if ('tokens' in token && Array.isArray(token.tokens)) visit(token.tokens, token.raw, absolute)
      if (token.type === 'list') visit(token.items, token.raw, absolute)
    }
  }
  visit(markdown.lexer(value), value, 0)
  return found.sort((left, right) => left.from - right.from)
}

function removedRanges(value: string, items: Reference[]): { from: number; to: number }[] {
  return items.map((item) => ({
    // Imports append separate Markdown blocks; their separator is hidden too.
    from: value.slice(0, item.from).endsWith('\n\n') ? item.from - 2 : item.from,
    to: item.to,
  }))
}

function withoutReferences(value: string, items: Reference[]): string {
  let result = value
  for (const { from, to } of removedRanges(value, items).reverse()) {
    result = result.slice(0, from) + result.slice(to)
  }
  return result
}

/** Project source scalar ranges into chip-free text, rejecting partially hidden spans. */
export function mapFieldAttachmentTextRanges<T extends { start: number; end: number }>(
  value: string, attachments: readonly KnownAttachment[], ranges: readonly T[],
): T[] {
  const removed = removedRanges(value, references(value, attachments)).map(({ from, to }) => ({
    from: [...value.slice(0, from)].length, to: [...value.slice(0, to)].length,
  }))
  const length = [...value].length
  return ranges.flatMap((range) => {
    if (!Number.isInteger(range.start) || !Number.isInteger(range.end) || range.start < 0 || range.end < range.start
      || range.end > length || removed.some(({ from, to }) => range.start < to && range.end > from)) return []
    const shift = removed.reduce((sum, { from, to }) => sum + (to <= range.start ? to - from : 0), 0)
    return [{ ...range, start: range.start - shift, end: range.end - shift }]
  })
}

export function fieldAttachmentText(value: string, attachments: readonly KnownAttachment[]) {
  const items = references(value, attachments)
  return {
    text: withoutReferences(value, items),
    attachmentIds: [...new Set(items.map((item) => item.attachmentId))],
  }
}

export function replaceFieldAttachmentText(value: string, text: string, attachments: readonly KnownAttachment[], limit: number): string {
  const items = references(value, attachments)
  if (text === withoutReferences(value, items)) return value
  const suffix = items.map((item) => item.raw).join('\n\n')
  const room = Math.max(0, limit - [...suffix].length - (suffix && text ? 2 : 0))
  const limited = [...text].slice(0, room).join('')
  return suffix ? `${limited}${limited ? '\n\n' : ''}${suffix}` : limited
}

export function removeFieldAttachment(value: string, attachmentId: string, attachments: readonly KnownAttachment[]): string {
  return withoutReferences(value, references(value, attachments).filter((item) => item.attachmentId === attachmentId))
}

/** A deleted request attachment must be unlinked from every editable field. */
export function removeWorkbenchAttachmentReferences(snapshot: FeedbackDraftSnapshot, attachmentId: string): FeedbackDraftSnapshot {
  return unlinkWorkbenchFieldAttachment(snapshot, attachmentId, (value) =>
    removeFieldAttachment(value, attachmentId, [{ attachment_id: attachmentId }]))
}

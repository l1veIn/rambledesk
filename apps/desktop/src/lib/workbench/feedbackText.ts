// Pure text and error helpers shared by the workbench components. Kept free
// of component state so they can be unit-tested and reused.

import type { Locale } from '../preferences'

export type CommandError = { message: string }

/** Append a markdown block to a draft body, separated by a blank line. */
export function appendMarkdownBlock(body: string, block: string): string {
  const current = body.trimEnd()
  return current ? `${current}\n\n${block}` : block
}

/**
 * Extract the Operator Feedback section from a cooked feedback document.
 * Returns the input unchanged unless it is a cooked document that starts
 * with a title and contains the Operator Feedback marker.
 */
export function operatorFeedbackBody(markdown: string): string {
  const marker = '\n## Operator Feedback\n\n'
  if (!markdown.startsWith('# ') || !markdown.includes(marker)) return markdown
  const body = markdown.slice(markdown.indexOf(marker) + marker.length)
  const attachments = body.indexOf('\n## Attachments\n\n')
  return attachments >= 0 ? body.slice(0, attachments).trimEnd() : body
}

/** Format a timestamp for the current locale; null/undefined means "not saved yet". */
export function formatTime(
  value: string | null | undefined,
  locale: Locale,
  notSavedLabel: string,
): string {
  if (!value) return notSavedLabel
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString(locale)
}

/** Extract a human-readable message from an unknown thrown value. */
export function messageFrom(cause: unknown): string {
  if (cause instanceof Error) return cause.message
  if (
    cause &&
    typeof cause === 'object' &&
    'message' in cause &&
    typeof (cause as CommandError).message === 'string'
  ) {
    return (cause as CommandError).message
  }
  return String(cause)
}

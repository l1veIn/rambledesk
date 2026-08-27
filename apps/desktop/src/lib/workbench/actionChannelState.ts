/**
 * Single source of truth for the currently selected Action channel per draft
 * request.
 *
 * The channel is owned by each FeedbackDraftSession; editors keep no copy of
 * it. Every programmatic insert (speech, captures, attachments, pasted
 * blocks) asks this module for the live value at insert time, so any editor
 * instance — visible, hidden owner, or fallback — stamps identically and can
 * never desync after remounts.
 */
const channels = new Map<string, number | null>()

export function rememberActionChannel(requestId: string, index: number | null): void {
  if (index == null) channels.delete(requestId)
  else channels.set(requestId, index)
}

export function forgetActionChannel(requestId: string): void {
  channels.delete(requestId)
}

export function actionChannelFor(requestId: string): number | null {
  return channels.get(requestId) ?? null
}

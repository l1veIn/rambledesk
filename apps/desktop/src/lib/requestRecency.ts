export const LAST_24_HOURS_MS = 24 * 60 * 60 * 1000

export function isWithinLast24Hours(updatedAt: string, now = Date.now()) {
  const stamp = new Date(updatedAt).getTime()
  return Number.isFinite(stamp) && stamp >= now - LAST_24_HOURS_MS
}

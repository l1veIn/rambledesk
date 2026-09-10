import type { SessionContextUsage } from '$lib/generated/feedback'

export function contextUsageDisplay(usage: SessionContextUsage | null | undefined) {
  if (!usage || !Number.isSafeInteger(usage.used) || usage.used < 0
    || !Number.isSafeInteger(usage.size) || usage.size <= 0) return null
  return {
    percent: Math.round(usage.used / usage.size * 100),
    used: usage.used.toLocaleString('en-US'),
    size: usage.size.toLocaleString('en-US'),
  }
}

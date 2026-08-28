export const MAX_TIDY_AUTO_THRESHOLD = 999

export function normalizeTidyAutoThreshold(value: number): number {
  if (!Number.isFinite(value)) return 0
  return Math.min(MAX_TIDY_AUTO_THRESHOLD, Math.max(0, Math.round(value)))
}

export function shouldAutoTidy(pendingCount: number, threshold: number): boolean {
  const normalized = normalizeTidyAutoThreshold(threshold)
  return normalized > 0 && pendingCount >= normalized
}

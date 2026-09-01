export const DESKTOP_NAVIGATION_POLL_INTERVAL_MS = 5_000

export function ensureDesktopNavigationPolling<Timer>(
  isDesktop: boolean,
  currentTimer: Timer | undefined,
  schedule: (callback: () => void, delayMs: number) => Timer,
  refresh: () => void,
): Timer | undefined {
  if (!isDesktop || currentTimer !== undefined) return currentTimer
  return schedule(refresh, DESKTOP_NAVIGATION_POLL_INTERVAL_MS)
}

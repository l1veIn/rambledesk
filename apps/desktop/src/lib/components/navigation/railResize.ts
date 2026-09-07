export type NavigationRail = 'host' | 'request'

export const RAIL_LIMITS = {
  host: { defaultWidth: 240, minWidth: 192, maxWidth: 360 },
  request: { defaultWidth: 240, minWidth: 200, maxWidth: 400 },
} as const

export const COLLAPSED_RAIL_WIDTH = 56

export function normalizeRailWidth(kind: NavigationRail, value: unknown): number {
  const { defaultWidth, minWidth, maxWidth } = RAIL_LIMITS[kind]
  if (typeof value !== 'number' || !Number.isFinite(value)) return defaultWidth
  return Math.min(maxWidth, Math.max(minWidth, value))
}

/** Collapsing keeps the expanded width from the start of this drag for later reopening. */
export function resolveRailDrag({
  initialWidth,
  initialExpandedWidth,
  delta,
  minWidth,
  maxWidth,
}: {
  initialWidth: number
  initialExpandedWidth: number
  delta: number
  minWidth: number
  maxWidth: number
}): { width: number; collapsed: boolean } {
  const raw = initialWidth + delta
  if (raw <= minWidth) return { width: initialExpandedWidth, collapsed: true }
  return { width: Math.min(maxWidth, Math.max(minWidth, raw)), collapsed: false }
}

/** Display-only fitting leaves preferred widths intact for a roomier window. */
export function fitNavigationWidths({
  hostWidth,
  requestWidth,
  hostCollapsed,
  requestCollapsed,
  containerWidth,
}: {
  hostWidth: number
  requestWidth: number | null
  hostCollapsed: boolean
  requestCollapsed: boolean
  containerWidth: number
}): { host: number; request: number } {
  let host = hostCollapsed ? COLLAPSED_RAIL_WIDTH : normalizeRailWidth('host', hostWidth)
  let request = requestWidth === null
    ? 0
    : requestCollapsed ? COLLAPSED_RAIL_WIDTH : normalizeRailWidth('request', requestWidth)
  const hostMinimum = hostCollapsed ? COLLAPSED_RAIL_WIDTH : RAIL_LIMITS.host.minWidth
  const requestMinimum = requestWidth === null
    ? 0
    : requestCollapsed ? COLLAPSED_RAIL_WIDTH : RAIL_LIMITS.request.minWidth
  const budget = Math.max(hostMinimum + requestMinimum, containerWidth - 640)
  const requestReduction = Math.min(request - requestMinimum, Math.max(0, host + request - budget))
  request -= requestReduction
  host -= Math.min(host - hostMinimum, Math.max(0, host + request - budget))
  return { host, request }
}

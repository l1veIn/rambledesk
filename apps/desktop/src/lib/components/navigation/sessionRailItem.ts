export type SessionOrigin = 'adapter' | 'managed_acp'

export type SessionRailItem = {
  key: string
  origin: SessionOrigin
  hostId: string
  sessionId: string
  title: string
  hostLabel: string
  hostIconSvg: string
  requestCount: number
  pendingCount: number
  updatedAt: string
  pinnedAt: string | null
  status: 'running' | 'waiting' | 'offline' | 'completed' | null
  canRename: boolean
  canPin: boolean
  canArchive: boolean
}

export function sessionRailKey(
  origin: SessionOrigin,
  hostId: string,
  sessionId: string,
): string {
  return `${origin}\u0000${hostId}\u0000${sessionId}`
}

export type SessionRailTotals = {
  requests: number
  pending: number
}

export type SessionRailActions = {
  rename: boolean
  pin: boolean
  archive: boolean
  any: boolean
}

function compareNullableIsoDesc(
  left: string | null | undefined,
  right: string | null | undefined,
): number {
  if (left === right) return 0
  if (!left) return 1
  if (!right) return -1
  return right.localeCompare(left)
}

export function compareSessionRailItems(left: SessionRailItem, right: SessionRailItem): number {
  return (
    compareNullableIsoDesc(left.pinnedAt, right.pinnedAt) ||
    compareNullableIsoDesc(left.updatedAt, right.updatedAt) ||
    left.key.localeCompare(right.key)
  )
}

export function orderSessionRailItems(items: readonly SessionRailItem[]): SessionRailItem[] {
  return [...items].sort(compareSessionRailItems)
}

export function sessionRailTotals(items: readonly SessionRailItem[]): SessionRailTotals {
  return items.reduce<SessionRailTotals>(
    (totals, item) => ({
      requests: totals.requests + item.requestCount,
      pending: totals.pending + item.pendingCount,
    }),
    { requests: 0, pending: 0 },
  )
}

export function sessionRailActions(item: SessionRailItem): SessionRailActions {
  return {
    rename: item.canRename,
    pin: item.canPin,
    archive: item.canArchive,
    any: item.canRename || item.canPin || item.canArchive,
  }
}

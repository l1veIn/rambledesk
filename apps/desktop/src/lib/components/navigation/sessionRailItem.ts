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

export type SessionRailStatusPresentation = {
  kind: 'running' | 'waiting' | 'offline' | 'completed' | 'error'
  label: 'Running' | 'Waiting for you' | 'Ramble Feedback' | 'Permission Request' |
    'Ask Question' | 'Offline' | 'Completed' | 'Operation failed'
  spinning: boolean
}

export function sessionRailStatusPresentation(
  status: string | null | undefined,
): SessionRailStatusPresentation | null {
  if (!status) return null
  const normalized = status.toLowerCase().replaceAll('-', '_')

  // A waiting subtype is a human hand-off, never background work. Keep this
  // branch before the running check so future compound statuses cannot show a
  // misleading spinner while the Agent is actually waiting for the user.
  if (normalized === 'waiting' || normalized.startsWith('waiting_')) {
    const label = normalized.includes('feedback')
      ? 'Ramble Feedback'
      : normalized.includes('permission')
        ? 'Permission Request'
        : normalized.includes('question') || normalized.includes('ask')
          ? 'Ask Question'
          : 'Waiting for you'
    return { kind: 'waiting', label, spinning: false }
  }

  if (normalized === 'running') {
    return { kind: 'running', label: 'Running', spinning: true }
  }
  if (normalized === 'offline') {
    return { kind: 'offline', label: 'Offline', spinning: false }
  }
  if (normalized === 'completed') {
    return { kind: 'completed', label: 'Completed', spinning: false }
  }
  if (normalized === 'error' || normalized === 'failed') {
    return { kind: 'error', label: 'Operation failed', spinning: false }
  }
  return null
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

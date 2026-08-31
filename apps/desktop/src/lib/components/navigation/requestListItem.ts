import type { SessionOrigin } from './sessionRailItem'

export type WorkbenchRequestKind = 'feedback' | 'permission' | 'question'

export type WorkbenchRequestStatus =
  | 'waiting'
  | 'in_progress'
  | 'completed'
  | 'cancelled'

/**
 * Presentation model for the middle request list.
 *
 * Wire DTOs are projected into this shape before they reach the navigation UI,
 * so the list does not need to know whether an item came from durable Feedback
 * storage or a live ACP request.
 */
export type WorkbenchRequestListItem = {
  /** Stable UI identity. Unlike the wire id, this includes origin and Session. */
  key: string
  origin: SessionOrigin
  rawRequestId: string
  sessionKey: string
  id: string
  kind: WorkbenchRequestKind
  title: string
  summary: string
  status: WorkbenchRequestStatus
  sessionId: string
  agentId: string
  sourceHint: string | null
  createdAt: string
  updatedAt: string
}

export function workbenchRequestKey(
  origin: SessionOrigin,
  sessionKey: string,
  rawRequestId: string,
): string {
  return `${origin}\u0000${sessionKey}\u0000${rawRequestId}`
}

export type RequestListAgentProfile = {
  id: string
  label: string
  iconSvg: string
}

export type WorkbenchRequestDisplayStatus = WorkbenchRequestStatus | 'cooking'

export function requestListItemStatusClass(status: WorkbenchRequestDisplayStatus): string {
  switch (status) {
    case 'cooking':
      return 'bg-primary/15 text-primary'
    case 'waiting':
      return 'bg-warning/15 text-warning-foreground dark:text-warning'
    case 'in_progress':
      return 'bg-info/15 text-info'
    case 'completed':
      return 'bg-success/15 text-success'
    case 'cancelled':
      return 'bg-destructive/12 text-destructive'
  }
}

export function requestListItemKindClass(kind: WorkbenchRequestKind): string {
  switch (kind) {
    case 'feedback':
      return 'bg-primary/12 text-primary'
    case 'permission':
      return 'bg-warning/15 text-warning-foreground dark:text-warning'
    case 'question':
      return 'bg-info/15 text-info'
  }
}

export function requestListItemKindLabel(kind: WorkbenchRequestKind): string {
  switch (kind) {
    case 'feedback':
      return 'Ramble Feedback'
    case 'permission':
      return 'Permission'
    case 'question':
      return 'Ask Question'
  }
}

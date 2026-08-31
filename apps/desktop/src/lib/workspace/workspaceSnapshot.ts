import {
  sessionViewDescriptor,
  settingsViewDescriptor,
  workspaceViewKey,
  type WorkspaceViewDescriptor,
} from './viewDescriptors'
import {
  EMPTY_WORKSPACE_SHELL_STATE,
  type WorkspaceShellState,
} from './workspaceShell'

export const WORKSPACE_SNAPSHOT_VERSION = 2 as const
export const MAX_WORKSPACE_SNAPSHOT_VIEWS = 50

const MAX_SNAPSHOT_ID_LENGTH = 512

export type WorkspaceSnapshotSessionViewV1 = Readonly<{
  kind: 'session'
  hostId: string
  hostSessionId: string
  lastRequestId: string | null
}>

export type WorkspaceSnapshotV1 = Readonly<{
  version: 1
  views: readonly WorkspaceSnapshotSessionViewV1[]
  activeViewKey: string | null
}>

export type WorkspaceSnapshotSessionViewV2 = WorkspaceSnapshotSessionViewV1

export type WorkspaceSnapshotSettingsViewV2 = Readonly<{
  kind: 'settings'
}>

export type WorkspaceSnapshotViewV2 =
  | WorkspaceSnapshotSessionViewV2
  | WorkspaceSnapshotSettingsViewV2

export type WorkspaceSnapshotV2 = Readonly<{
  version: typeof WORKSPACE_SNAPSHOT_VERSION
  views: readonly WorkspaceSnapshotViewV2[]
  activeViewKey: string | null
}>

export type RestoredWorkspaceSnapshot = Readonly<{
  shellState: WorkspaceShellState
  requestIds: ReadonlyMap<string, string>
}>

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function validId(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0 && value.length <= MAX_SNAPSHOT_ID_LENGTH
}

function parseSessionView(value: unknown): WorkspaceSnapshotSessionViewV1 | null {
  if (!isRecord(value) || value.kind !== 'session') return null
  if (!validId(value.hostId) || !validId(value.hostSessionId)) return null
  const lastRequestId = validId(value.lastRequestId) ? value.lastRequestId : null
  return {
    kind: 'session',
    hostId: value.hostId,
    hostSessionId: value.hostSessionId,
    lastRequestId,
  }
}

function descriptorForSnapshotView(
  value: unknown,
  version: 1 | typeof WORKSPACE_SNAPSHOT_VERSION,
): Readonly<{
  view: WorkspaceViewDescriptor
  lastRequestId: string | null
}> | null {
  const session = parseSessionView(value)
  if (session) {
    return {
      view: sessionViewDescriptor(session.hostId, session.hostSessionId),
      lastRequestId: session.lastRequestId,
    }
  }
  if (version === WORKSPACE_SNAPSHOT_VERSION && isRecord(value) && value.kind === 'settings') {
    return { view: settingsViewDescriptor(), lastRequestId: null }
  }
  return null
}

export function createWorkspaceSnapshot(
  state: WorkspaceShellState,
  requestIds: ReadonlyMap<string, string>,
): WorkspaceSnapshotV2 {
  const views = state.views
    .slice(0, MAX_WORKSPACE_SNAPSHOT_VIEWS)
    .map((view): WorkspaceSnapshotViewV2 =>
      view.kind === 'session'
        ? {
            ...view,
            lastRequestId: requestIds.get(workspaceViewKey(view)) ?? null,
          }
        : { kind: 'settings' },
    )
  const knownKeys = new Set(state.views.slice(0, MAX_WORKSPACE_SNAPSHOT_VIEWS).map(workspaceViewKey))
  return {
    version: WORKSPACE_SNAPSHOT_VERSION,
    views,
    activeViewKey:
      state.activeViewKey && knownKeys.has(state.activeViewKey)
        ? state.activeViewKey
        : views[0]
          ? workspaceViewKey(
              views[0].kind === 'session'
                ? sessionViewDescriptor(views[0].hostId, views[0].hostSessionId)
                : settingsViewDescriptor(),
            )
          : null,
  }
}

export function restoreWorkspaceSnapshot(value: unknown): RestoredWorkspaceSnapshot | null {
  if (
    !isRecord(value) ||
    (value.version !== 1 && value.version !== WORKSPACE_SNAPSHOT_VERSION)
  ) return null
  if (!Array.isArray(value.views)) return null

  const views: WorkspaceViewDescriptor[] = []
  const requestIds = new Map<string, string>()
  const knownKeys = new Set<string>()

  for (const candidate of value.views) {
    if (views.length >= MAX_WORKSPACE_SNAPSHOT_VIEWS) break
    const parsed = descriptorForSnapshotView(candidate, value.version)
    if (!parsed) continue
    const view = parsed.view
    const viewKey = workspaceViewKey(view)
    if (knownKeys.has(viewKey)) continue
    knownKeys.add(viewKey)
    views.push(view)
    if (view.kind === 'session' && parsed.lastRequestId) {
      requestIds.set(viewKey, parsed.lastRequestId)
    }
  }

  if (views.length === 0) {
    return { shellState: EMPTY_WORKSPACE_SHELL_STATE, requestIds }
  }
  const requestedActiveViewKey =
    typeof value.activeViewKey === 'string' && knownKeys.has(value.activeViewKey)
      ? value.activeViewKey
      : null
  return {
    shellState: {
      views,
      activeViewKey: requestedActiveViewKey ?? workspaceViewKey(views[0]),
    },
    requestIds,
  }
}

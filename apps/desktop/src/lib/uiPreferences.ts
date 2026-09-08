import {
  restoreWorkspaceSnapshot,
  type RestoredWorkspaceSnapshot,
  type WorkspaceSnapshotV2,
} from './workspace/workspaceSnapshot'
import { normalizeRailWidth } from './components/navigation/railResize'

export type UiThemePreference = 'system' | 'light' | 'dark'

type UiState = {
  theme?: UiThemePreference
  workbench?: {
    hostRailCollapsed?: boolean
    requestRailCollapsed?: boolean
    hostRailWidth?: number
    requestRailWidth?: number
    paneLayouts?: Record<string, number[]>
    workspaceSnapshot?: unknown
  }
  webAccess?: {
    port?: number
    autostart?: boolean
  }
}

/** The documented Web Access entry; the browser server settings may override it. */
export const DEFAULT_WEB_ACCESS_PORT = 37643
export const WEB_ACCESS_PORT_MIN = 1024
export const WEB_ACCESS_PORT_MAX = 65535

export function normalizeWebAccessPort(value: unknown): number {
  return typeof value === 'number' &&
    Number.isInteger(value) &&
    value >= WEB_ACCESS_PORT_MIN &&
    value <= WEB_ACCESS_PORT_MAX
    ? value
    : DEFAULT_WEB_ACCESS_PORT
}

const UI_STATE_KEY = 'rambledesk.ui-state'

function readState(): UiState {
  if (typeof localStorage === 'undefined') return {}
  try {
    const raw = localStorage.getItem(UI_STATE_KEY)
    if (!raw) return {}
    const value: unknown = JSON.parse(raw)
    if (!value || typeof value !== 'object' || Array.isArray(value)) return {}
    const state = value as UiState
    if (state.workbench !== undefined && (
      !state.workbench || typeof state.workbench !== 'object' || Array.isArray(state.workbench)
    )) {
      delete state.workbench
    }
    return state
  } catch {
    return {}
  }
}

function updateState(update: (state: UiState) => void) {
  if (typeof localStorage === 'undefined') return
  try {
    const state = readState()
    update(state)
    localStorage.setItem(UI_STATE_KEY, JSON.stringify(state))
  } catch {
    // UI preferences are optional and must never prevent the workbench from opening.
  }
}

export function savedUiTheme(): UiThemePreference | null {
  const theme = readState().theme
  return theme === 'system' || theme === 'light' || theme === 'dark' ? theme : null
}

export function saveUiTheme(theme: UiThemePreference) {
  updateState((state) => {
    state.theme = theme
  })
}

export function initialHostRailCollapsed() {
  return readState().workbench?.hostRailCollapsed === true
}

export function saveHostRailCollapsed(collapsed: boolean) {
  updateState((state) => {
    state.workbench ??= {}
    state.workbench.hostRailCollapsed = collapsed
  })
}

export function initialRequestRailCollapsed() {
  return readState().workbench?.requestRailCollapsed === true
}

export function saveRequestRailCollapsed(collapsed: boolean) {
  updateState((state) => {
    state.workbench ??= {}
    state.workbench.requestRailCollapsed = collapsed
  })
}

export function initialHostRailWidth(): number {
  return normalizeRailWidth('host', readState().workbench?.hostRailWidth)
}

export function saveHostRailWidth(width: number) {
  updateState((state) => {
    state.workbench ??= {}
    state.workbench.hostRailWidth = normalizeRailWidth('host', width)
  })
}

export function initialRequestRailWidth(): number {
  return normalizeRailWidth('request', readState().workbench?.requestRailWidth)
}

export function saveRequestRailWidth(width: number) {
  updateState((state) => {
    state.workbench ??= {}
    state.workbench.requestRailWidth = normalizeRailWidth('request', width)
  })
}

export function initialWebAccessPort(): number {
  return normalizeWebAccessPort(readState().webAccess?.port)
}

export function saveWebAccessPort(port: number) {
  updateState((state) => {
    state.webAccess ??= {}
    state.webAccess.port = normalizeWebAccessPort(port)
  })
}

export function initialWebAccessAutostart(): boolean {
  return readState().webAccess?.autostart === true
}

export function saveWebAccessAutostart(autostart: boolean) {
  updateState((state) => {
    state.webAccess ??= {}
    state.webAccess.autostart = autostart
  })
}

export function savedPaneLayout(key: string): number[] | null {
  const layout = readState().workbench?.paneLayouts?.[key]
  if (
    !Array.isArray(layout) ||
    layout.length < 2 ||
    layout.some((size) => typeof size !== 'number' || !Number.isFinite(size) || size < 0)
  ) {
    return null
  }
  return [...layout]
}

/** Every workbench adjustment lives under the same localStorage record. */
export function savePaneLayout(key: string, layout: number[]) {
  if (
    layout.length < 2 ||
    layout.some((size) => !Number.isFinite(size) || size < 0)
  ) {
    return
  }
  updateState((state) => {
    state.workbench ??= {}
    state.workbench.paneLayouts ??= {}
    state.workbench.paneLayouts[key] = [...layout]
  })
}

export function savedWorkspaceSnapshot(): RestoredWorkspaceSnapshot | null {
  return restoreWorkspaceSnapshot(readState().workbench?.workspaceSnapshot)
}

export function saveWorkspaceSnapshot(snapshot: WorkspaceSnapshotV2) {
  updateState((state) => {
    state.workbench ??= {}
    state.workbench.workspaceSnapshot = snapshot
  })
}

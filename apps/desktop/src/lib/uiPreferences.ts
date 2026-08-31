import {
  restoreWorkspaceSnapshot,
  type RestoredWorkspaceSnapshot,
  type WorkspaceSnapshotV2,
} from './workspace/workspaceSnapshot'

export type UiThemePreference = 'system' | 'light' | 'dark'

type UiState = {
  theme?: UiThemePreference
  workbench?: {
    hostRailCollapsed?: boolean
    paneLayouts?: Record<string, number[]>
    workspaceSnapshot?: unknown
  }
}

const UI_STATE_KEY = 'rambledesk.ui-state'

function readState(): UiState {
  if (typeof localStorage === 'undefined') return {}
  try {
    const raw = localStorage.getItem(UI_STATE_KEY)
    if (!raw) return {}
    const value: unknown = JSON.parse(raw)
    return value && typeof value === 'object' ? (value as UiState) : {}
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

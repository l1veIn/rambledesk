import {
  workspaceViewKey,
  type WorkspaceViewDescriptor,
} from './viewDescriptors'

export type WorkspaceShellState = Readonly<{
  views: readonly WorkspaceViewDescriptor[]
  activeViewKey: string | null
}>

export type WorkspaceShellAction =
  | Readonly<{ type: 'open'; view: WorkspaceViewDescriptor }>
  | Readonly<{ type: 'focus'; viewKey: string }>
  | Readonly<{ type: 'close'; viewKey: string }>

const EMPTY_WORKSPACE_VIEWS: readonly WorkspaceViewDescriptor[] = Object.freeze([])

export const EMPTY_WORKSPACE_SHELL_STATE: WorkspaceShellState = Object.freeze({
  views: EMPTY_WORKSPACE_VIEWS,
  activeViewKey: null,
})

export function workspaceShellReducer(
  state: WorkspaceShellState,
  action: WorkspaceShellAction,
): WorkspaceShellState {
  switch (action.type) {
    case 'open': {
      const viewKey = workspaceViewKey(action.view)
      const alreadyOpen = state.views.some((view) => workspaceViewKey(view) === viewKey)
      if (alreadyOpen) {
        return state.activeViewKey === viewKey ? state : { ...state, activeViewKey: viewKey }
      }
      return {
        views: [...state.views, action.view],
        activeViewKey: viewKey,
      }
    }
    case 'focus': {
      if (state.activeViewKey === action.viewKey) return state
      const known = state.views.some((view) => workspaceViewKey(view) === action.viewKey)
      return known ? { ...state, activeViewKey: action.viewKey } : state
    }
    case 'close': {
      const closedIndex = state.views.findIndex(
        (view) => workspaceViewKey(view) === action.viewKey,
      )
      if (closedIndex === -1) return state

      const views = state.views.filter((_, index) => index !== closedIndex)
      if (state.activeViewKey !== action.viewKey) return { ...state, views }

      const fallback = views[Math.min(closedIndex, views.length - 1)]
      return {
        views,
        activeViewKey: fallback ? workspaceViewKey(fallback) : null,
      }
    }
  }
}

export function activeWorkspaceView(
  state: WorkspaceShellState,
): WorkspaceViewDescriptor | null {
  if (!state.activeViewKey) return null
  return state.views.find((view) => workspaceViewKey(view) === state.activeViewKey) ?? null
}

import { get, writable } from 'svelte/store'

import {
  EMPTY_WORKSPACE_SHELL_STATE,
  activeWorkspaceView,
  workspaceShellReducer,
  type WorkspaceShellAction,
  type WorkspaceShellState,
} from '../workspace/workspaceShell'
import {
  createWorkspaceSnapshot,
  type RestoredWorkspaceSnapshot,
  type WorkspaceSnapshotV2,
} from '../workspace/workspaceSnapshot'
import { workspaceViewKey, type WorkspaceViewDescriptor } from '../workspace/viewDescriptors'
import { saveWorkspaceSnapshot, savedWorkspaceSnapshot } from '../uiPreferences'

/**
 * Which workspace views are open, which one is active, and which request each session
 * view shows. Persisted so the workbench reopens where the human left it.
 */
export type WorkspaceShellSessionState = Readonly<{
  shell: WorkspaceShellState
  requestIds: ReadonlyMap<string, string>
  pendingViewKey: string | null
}>

/** Where the open-view snapshot is stored; the preview workbench injects its own. */
export type WorkspaceSnapshotStore = Readonly<{
  load: () => RestoredWorkspaceSnapshot | null
  save: (snapshot: WorkspaceSnapshotV2) => void
}>

const defaultSnapshotStore: WorkspaceSnapshotStore = {
  load: savedWorkspaceSnapshot,
  save: saveWorkspaceSnapshot,
}

export type WorkspaceShellSession = ReturnType<typeof createWorkspaceShellSession>

export function createWorkspaceShellSession(
  options: { snapshots?: WorkspaceSnapshotStore } = {},
) {
  const snapshots = options.snapshots ?? defaultSnapshotStore
  const restored = snapshots.load()
  const store = writable<WorkspaceShellSessionState>({
    shell: restored?.shellState ?? EMPTY_WORKSPACE_SHELL_STATE,
    requestIds: new Map(restored?.requestIds ?? []),
    pendingViewKey: null,
  })

  function patch(next: Partial<WorkspaceShellSessionState>) {
    store.update((current) => ({ ...current, ...next }))
  }

  /** Persists the current open views so the next launch restores them. */
  function persist() {
    const state = get(store)
    const snapshot = createWorkspaceSnapshot(state.shell, state.requestIds)
    snapshots.save(snapshot)
  }

  /** Applies a shell action; returns whether the open views changed. */
  function dispatch(action: WorkspaceShellAction): boolean {
    const shell = get(store).shell
    const next = workspaceShellReducer(shell, action)
    if (next === shell) return false
    patch({ shell: next })
    persist()
    return true
  }

  /** Applies an externally computed shell state, for example after a cleanup pass. */
  function replaceShell(shell: WorkspaceShellState) {
    patch({ shell })
    persist()
  }

  /** Remembers the request a session view shows, so reopening restores it. */
  function bindRequest(viewKey: string, requestId: string) {
    patch({ requestIds: new Map(get(store).requestIds).set(viewKey, requestId) })
    persist()
  }

  function forgetRequest(viewKey: string) {
    const requestIds = new Map(get(store).requestIds)
    if (!requestIds.delete(viewKey)) return
    patch({ requestIds })
    persist()
  }

  function setPendingViewKey(pendingViewKey: string | null) {
    patch({ pendingViewKey })
  }

  function views(): readonly WorkspaceViewDescriptor[] {
    return get(store).shell.views
  }

  function activeView(): WorkspaceViewDescriptor | null {
    return activeWorkspaceView(get(store).shell)
  }

  function activeViewKey(): string | null {
    return get(store).shell.activeViewKey
  }

  function pendingViewKey(): string | null {
    return get(store).pendingViewKey
  }

  function requestIdFor(view: WorkspaceViewDescriptor | string): string | undefined {
    const viewKey = typeof view === 'string' ? view : workspaceViewKey(view)
    return get(store).requestIds.get(viewKey)
  }

  /** True when the saved snapshot had an open view, so the first mount can wait. */
  function restoredActiveView(): boolean {
    return restored?.shellState.activeViewKey != null
  }

  /** True when this launch restored any saved workspace state. */
  function hasRestoredSnapshot(): boolean {
    return restored !== null
  }

  return {
    subscribe: store.subscribe,
    dispatch,
    replaceShell,
    bindRequest,
    forgetRequest,
    setPendingViewKey,
    persist,
    views,
    activeView,
    activeViewKey,
    pendingViewKey,
    requestIdFor,
    restoredActiveView,
    hasRestoredSnapshot,
  }
}

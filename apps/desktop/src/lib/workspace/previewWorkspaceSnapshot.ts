import {
  restoreWorkspaceSnapshot,
  type RestoredWorkspaceSnapshot,
  type WorkspaceSnapshotV2,
} from './workspaceSnapshot'
import {
  sessionViewDescriptor,
  settingsViewDescriptor,
  workspaceViewKey,
} from './viewDescriptors'

export type PreviewWorkspaceScenario =
  | 'restore'
  | 'archived'
  | 'unavailable'
  | 'unknown'
  | 'settings'

let snapshot: WorkspaceSnapshotV2 | null = null

export function savedPreviewWorkspaceSnapshot(): RestoredWorkspaceSnapshot | null {
  return restoreWorkspaceSnapshot(snapshot)
}

export function savePreviewWorkspaceSnapshot(next: WorkspaceSnapshotV2) {
  snapshot = next
}

export function seedPreviewWorkspaceScenario(value: string | null): PreviewWorkspaceScenario | null {
  if (
    value !== 'restore' &&
    value !== 'archived' &&
    value !== 'unavailable' &&
    value !== 'unknown' &&
    value !== 'settings'
  ) {
    return null
  }
  if (value === 'settings') {
    const view = settingsViewDescriptor()
    savePreviewWorkspaceSnapshot({
      version: 2,
      views: [view],
      activeViewKey: workspaceViewKey(view),
    })
    return value
  }
  const view =
    value === 'restore'
      ? sessionViewDescriptor('codex', 'desktop-refactor-2026-08-02')
      : value === 'archived'
        ? sessionViewDescriptor('codex', 'archived-preview-session')
        : sessionViewDescriptor('codex', 'unavailable-preview-session')
  savePreviewWorkspaceSnapshot({
    version: 2,
    views: [
      {
        ...view,
        lastRequestId:
          value === 'restore' ? '019fc1d9-51e7-7eb2-b196-e9266947fc41' : null,
      },
    ],
    activeViewKey: workspaceViewKey(view),
  })
  return value
}

export function resetPreviewWorkspaceSnapshot() {
  snapshot = null
}

import {
  restoreWorkspaceSnapshot,
  type RestoredWorkspaceSnapshot,
  type WorkspaceSnapshotV1,
} from './workspaceSnapshot'
import { sessionViewDescriptor, workspaceViewKey } from './viewDescriptors'

export type PreviewWorkspaceScenario = 'restore' | 'archived' | 'unavailable' | 'unknown'

let snapshot: WorkspaceSnapshotV1 | null = null

export function savedPreviewWorkspaceSnapshot(): RestoredWorkspaceSnapshot | null {
  return restoreWorkspaceSnapshot(snapshot)
}

export function savePreviewWorkspaceSnapshot(next: WorkspaceSnapshotV1) {
  snapshot = next
}

export function seedPreviewWorkspaceScenario(value: string | null): PreviewWorkspaceScenario | null {
  if (value !== 'restore' && value !== 'archived' && value !== 'unavailable' && value !== 'unknown') {
    return null
  }
  const view =
    value === 'restore'
      ? sessionViewDescriptor('codex', 'desktop-refactor-2026-08-02')
      : value === 'archived'
        ? sessionViewDescriptor('codex', 'archived-preview-session')
        : sessionViewDescriptor('codex', 'unavailable-preview-session')
  savePreviewWorkspaceSnapshot({
    version: 1,
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

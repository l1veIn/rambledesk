import {
  restoreWorkspaceSnapshot,
  type RestoredWorkspaceSnapshot,
  type WorkspaceSnapshotV2,
} from './workspaceSnapshot'
import {
  inboxViewDescriptor,
  rambelleProfileViewDescriptor,
  requestTaskViewDescriptor,
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
  | 'task'
  | 'profile'
  | 'tabs'

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
    value !== 'settings' &&
    value !== 'task' &&
    value !== 'profile' &&
    value !== 'tabs'
  ) {
    return null
  }
  if (value === 'tabs') {
    const activeView = sessionViewDescriptor('codex', 'desktop-refactor-2026-08-02')
    const views: WorkspaceSnapshotV2['views'] = [
      inboxViewDescriptor(),
      { ...activeView, lastRequestId: '019fc1d9-51e7-7eb2-b196-e9266947fc41' },
      { ...sessionViewDescriptor('pi', 'pi-native-wait'), lastRequestId: null },
      { ...sessionViewDescriptor('claude', 'terminology-audit'), lastRequestId: null },
      requestTaskViewDescriptor('019fc1d9-51e7-7eb2-b196-e9266947fc41'),
      rambelleProfileViewDescriptor(),
      settingsViewDescriptor(),
      { ...sessionViewDescriptor('codex', 'release-readiness'), lastRequestId: null },
      { ...sessionViewDescriptor('pi', 'native-capture-review'), lastRequestId: null },
      { ...sessionViewDescriptor('claude', 'adapter-contract-audit'), lastRequestId: null },
      { ...sessionViewDescriptor('codex', 'web-service-follow-up'), lastRequestId: null },
      { ...sessionViewDescriptor('pi', 'feedback-draft-polish'), lastRequestId: null },
    ]
    savePreviewWorkspaceSnapshot({
      version: 2,
      views,
      activeViewKey: workspaceViewKey(activeView),
    })
    return value
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
  if (value === 'task' || value === 'profile') {
    const view = value === 'task'
      ? requestTaskViewDescriptor('019fc1d9-51e7-7eb2-b196-e9266947fc41')
      : rambelleProfileViewDescriptor()
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

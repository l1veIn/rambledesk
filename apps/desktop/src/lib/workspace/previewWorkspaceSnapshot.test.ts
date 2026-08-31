import { afterEach, describe, expect, it } from 'vitest'

import { sessionViewDescriptor, workspaceViewKey } from './viewDescriptors'
import {
  resetPreviewWorkspaceSnapshot,
  savedPreviewWorkspaceSnapshot,
  savePreviewWorkspaceSnapshot,
  seedPreviewWorkspaceScenario,
} from './previewWorkspaceSnapshot'

afterEach(resetPreviewWorkspaceSnapshot)

describe('preview workspace snapshot store', () => {
  it('keeps preview navigation in memory without browser storage', () => {
    const view = sessionViewDescriptor('codex', 'preview-session')
    savePreviewWorkspaceSnapshot({
      version: 2,
      views: [{ ...view, lastRequestId: 'preview-request' }],
      activeViewKey: workspaceViewKey(view),
    })

    expect(savedPreviewWorkspaceSnapshot()?.shellState).toEqual({
      views: [view],
      activeViewKey: workspaceViewKey(view),
    })
    expect(savedPreviewWorkspaceSnapshot()?.requestIds.get(workspaceViewKey(view))).toBe(
      'preview-request',
    )
  })

  it.each([
    ['restore', 'desktop-refactor-2026-08-02', '019fc1d9-51e7-7eb2-b196-e9266947fc41'],
    ['archived', 'archived-preview-session', null],
    ['unavailable', 'unavailable-preview-session', null],
    ['unknown', 'unavailable-preview-session', null],
  ] as const)('seeds the %s browser acceptance scenario', (scenario, sessionId, requestId) => {
    expect(seedPreviewWorkspaceScenario(scenario)).toBe(scenario)

    const restored = savedPreviewWorkspaceSnapshot()!
    expect(restored.shellState.views).toEqual([sessionViewDescriptor('codex', sessionId)])
    expect(restored.requestIds.get(workspaceViewKey(restored.shellState.views[0])) ?? null).toBe(
      requestId,
    )
  })

  it('ignores unknown browser acceptance scenarios', () => {
    expect(seedPreviewWorkspaceScenario('other')).toBeNull()
    expect(savedPreviewWorkspaceSnapshot()).toBeNull()
  })

  it('seeds settings as a singleton workspace view without transient section state', () => {
    expect(seedPreviewWorkspaceScenario('settings')).toBe('settings')
    expect(savedPreviewWorkspaceSnapshot()?.shellState).toEqual({
      views: [{ kind: 'settings' }],
      activeViewKey: 'settings:singleton',
    })
    expect(savedPreviewWorkspaceSnapshot()?.requestIds.size).toBe(0)
  })

  it.each([
    ['task', { kind: 'request-task', requestId: '019fc1d9-51e7-7eb2-b196-e9266947fc41' }],
    ['profile', { kind: 'rambelle-profile' }],
  ] as const)('seeds the %s non-session workspace scenario', (scenario, view) => {
    expect(seedPreviewWorkspaceScenario(scenario)).toBe(scenario)
    expect(savedPreviewWorkspaceSnapshot()?.shellState).toEqual({
      views: [view],
      activeViewKey:
        scenario === 'task'
          ? 'request-task:"019fc1d9-51e7-7eb2-b196-e9266947fc41"'
          : 'rambelle-profile:singleton',
    })
  })
})

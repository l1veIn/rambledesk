import { afterEach, describe, expect, it, vi } from 'vitest'

import { sessionViewDescriptor, workspaceViewKey } from './workspace/viewDescriptors'
import { workspaceShellReducer, EMPTY_WORKSPACE_SHELL_STATE } from './workspace/workspaceShell'
import { createWorkspaceSnapshot } from './workspace/workspaceSnapshot'

function memoryStorage(initial: Record<string, string> = {}): Storage {
  const values = new Map(Object.entries(initial))
  return {
    get length() {
      return values.size
    },
    clear: () => values.clear(),
    getItem: (key) => values.get(key) ?? null,
    key: (index) => [...values.keys()][index] ?? null,
    removeItem: (key) => values.delete(key),
    setItem: (key, value) => values.set(key, value),
  }
}

afterEach(() => {
  vi.unstubAllGlobals()
  vi.resetModules()
})

describe('workspace snapshot preferences', () => {
  it('saves and restores a snapshot without replacing existing UI preferences', async () => {
    const storage = memoryStorage({
      'rambledesk.ui-state': JSON.stringify({
        theme: 'dark',
        workbench: { paneLayouts: { primary: [25, 75] } },
      }),
    })
    vi.stubGlobal('localStorage', storage)
    const preferences = await import('./uiPreferences')
    const view = sessionViewDescriptor('codex', 'alpha')
    const state = workspaceShellReducer(EMPTY_WORKSPACE_SHELL_STATE, { type: 'open', view })

    preferences.saveWorkspaceSnapshot(
      createWorkspaceSnapshot(state, new Map([[workspaceViewKey(view), 'request-1']])),
    )

    expect(preferences.savedWorkspaceSnapshot()?.shellState).toEqual(state)
    expect(preferences.savedWorkspaceSnapshot()?.requestIds.get(workspaceViewKey(view))).toBe(
      'request-1',
    )
    expect(JSON.parse(storage.getItem('rambledesk.ui-state')!)).toMatchObject({
      theme: 'dark',
      workbench: { paneLayouts: { primary: [25, 75] } },
    })
  })

  it('ignores corrupt snapshots while preserving other readable preferences', async () => {
    vi.stubGlobal(
      'localStorage',
      memoryStorage({
        'rambledesk.ui-state': JSON.stringify({
          theme: 'light',
          workbench: { workspaceSnapshot: { version: 99, views: [] } },
        }),
      }),
    )
    const preferences = await import('./uiPreferences')

    expect(preferences.savedWorkspaceSnapshot()).toBeNull()
    expect(preferences.savedUiTheme()).toBe('light')
  })

  it('treats storage failures as optional UI preference failures', async () => {
    const storage = memoryStorage()
    storage.setItem = () => {
      throw new Error('quota exceeded')
    }
    vi.stubGlobal('localStorage', storage)
    const preferences = await import('./uiPreferences')

    expect(() =>
      preferences.saveWorkspaceSnapshot({ version: 2, views: [], activeViewKey: null }),
    ).not.toThrow()
    expect(preferences.savedWorkspaceSnapshot()).toBeNull()
  })
})

describe('navigation rail width preferences', () => {
  it('starts both expanded rails at 240px when storage is absent or unavailable', async () => {
    vi.stubGlobal('localStorage', undefined)
    const preferences = await import('./uiPreferences')
    expect(preferences.initialHostRailWidth()).toBe(240)
    expect(preferences.initialRequestRailWidth()).toBe(240)
    expect(() => preferences.saveHostRailWidth(280)).not.toThrow()
    expect(() => preferences.saveRequestRailWidth(320)).not.toThrow()
  })

  it('normalizes saved widths without coercing malformed types', async () => {
    const storage = memoryStorage()
    vi.stubGlobal('localStorage', storage)
    const preferences = await import('./uiPreferences')
    for (const value of [null, '310', true, [], {}]) {
      storage.setItem('rambledesk.ui-state', JSON.stringify({
        workbench: { hostRailWidth: value, requestRailWidth: value },
      }))
      expect(preferences.initialHostRailWidth()).toBe(240)
      expect(preferences.initialRequestRailWidth()).toBe(240)
    }
    storage.setItem('rambledesk.ui-state', JSON.stringify({
      workbench: { hostRailWidth: 999, requestRailWidth: 56 },
    }))
    expect(preferences.initialHostRailWidth()).toBe(360)
    expect(preferences.initialRequestRailWidth()).toBe(200)
  })

  it('preserves expanded widths across collapse toggles and subsequent preference writes', async () => {
    const snapshot = { version: 2, views: [], activeViewKey: null }
    const storage = memoryStorage({
      'rambledesk.ui-state': JSON.stringify({
        theme: 'dark',
        workbench: { paneLayouts: { primary: [25, 75] }, workspaceSnapshot: snapshot },
      }),
    })
    vi.stubGlobal('localStorage', storage)
    const preferences = await import('./uiPreferences')
    preferences.saveHostRailWidth(275)
    preferences.saveRequestRailWidth(320)
    preferences.saveHostRailCollapsed(true)
    preferences.saveRequestRailCollapsed(true)
    preferences.savePaneLayout('secondary', [40, 60])

    expect(preferences.initialHostRailWidth()).toBe(275)
    expect(preferences.initialRequestRailWidth()).toBe(320)
    expect(JSON.parse(storage.getItem('rambledesk.ui-state')!)).toEqual({
      theme: 'dark',
      workbench: {
        hostRailWidth: 275,
        requestRailWidth: 320,
        hostRailCollapsed: true,
        requestRailCollapsed: true,
        paneLayouts: { primary: [25, 75], secondary: [40, 60] },
        workspaceSnapshot: snapshot,
      },
    })
    preferences.saveHostRailCollapsed(false)
    preferences.saveRequestRailCollapsed(false)
    expect(preferences.initialHostRailWidth()).toBe(275)
    expect(preferences.initialRequestRailWidth()).toBe(320)
  })

  it('persists normalized expanded widths rather than collapsed or out-of-range values', async () => {
    vi.stubGlobal('localStorage', memoryStorage())
    const preferences = await import('./uiPreferences')
    preferences.saveHostRailWidth(56)
    preferences.saveRequestRailWidth(999)
    expect(preferences.initialHostRailWidth()).toBe(192)
    expect(preferences.initialRequestRailWidth()).toBe(400)
    preferences.saveHostRailWidth(NaN)
    preferences.saveRequestRailWidth(Infinity)
    expect(preferences.initialHostRailWidth()).toBe(240)
    expect(preferences.initialRequestRailWidth()).toBe(240)
  })

  it('recovers malformed stored records so the next width adjustment can be saved', async () => {
    const storage = memoryStorage()
    vi.stubGlobal('localStorage', storage)
    const preferences = await import('./uiPreferences')
    for (const raw of ['{broken', 'null', '[]', 'true', '{"theme":"light","workbench":42}', '{"theme":"light","workbench":[]}']) {
      storage.setItem('rambledesk.ui-state', raw)
      expect(preferences.initialHostRailWidth()).toBe(240)
      expect(preferences.initialRequestRailWidth()).toBe(240)
      preferences.saveHostRailWidth(280)
      preferences.saveRequestRailWidth(300)
      expect(preferences.initialHostRailWidth()).toBe(280)
      expect(preferences.initialRequestRailWidth()).toBe(300)
      if (raw.includes('"light"')) expect(preferences.savedUiTheme()).toBe('light')
    }
  })

  it('leaves the last saved widths intact when persistence fails', async () => {
    const storage = memoryStorage({
      'rambledesk.ui-state': JSON.stringify({ workbench: { hostRailWidth: 280, requestRailWidth: 300 } }),
    })
    storage.setItem = () => { throw new Error('quota exceeded') }
    vi.stubGlobal('localStorage', storage)
    const preferences = await import('./uiPreferences')
    expect(() => preferences.saveHostRailWidth(330)).not.toThrow()
    expect(() => preferences.saveRequestRailWidth(350)).not.toThrow()
    expect(preferences.initialHostRailWidth()).toBe(280)
    expect(preferences.initialRequestRailWidth()).toBe(300)
  })
})

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

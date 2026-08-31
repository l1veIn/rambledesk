import { describe, expect, it } from 'vitest'

import {
  rambelleProfileViewDescriptor,
  sessionViewDescriptor,
  settingsViewDescriptor,
} from './viewDescriptors'
import { leavesSettingsView } from './workspaceViewLifecycle'

describe('workspace view lifecycle', () => {
  it('detects every successful transition away from settings', () => {
    expect(leavesSettingsView(settingsViewDescriptor(), sessionViewDescriptor('codex', 'one'))).toBe(true)
    expect(leavesSettingsView(settingsViewDescriptor(), rambelleProfileViewDescriptor())).toBe(true)
    expect(leavesSettingsView(settingsViewDescriptor(), null)).toBe(true)
  })

  it('does not report opening, remaining in, or unrelated transitions', () => {
    expect(leavesSettingsView(null, settingsViewDescriptor())).toBe(false)
    expect(leavesSettingsView(settingsViewDescriptor(), settingsViewDescriptor())).toBe(false)
    expect(leavesSettingsView(sessionViewDescriptor('codex', 'one'), null)).toBe(false)
  })
})

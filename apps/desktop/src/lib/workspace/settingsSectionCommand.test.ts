import { describe, expect, it } from 'vitest'

import { applySettingsSectionCommand } from './settingsSectionCommand'

describe('settings section commands', () => {
  it('re-applies the same deep-linked section when a newer command arrives', () => {
    const initial = { activeSection: 'voice' as const, appliedEpoch: 1 }
    const repeated = applySettingsSectionCommand(initial, 'voice', 2)

    expect(repeated).toEqual({ activeSection: 'voice', appliedEpoch: 2 })
    expect(repeated).not.toBe(initial)
  })

  it('ignores an already-applied command and applies a different section', () => {
    const initial = { activeSection: 'general' as const, appliedEpoch: 4 }

    expect(applySettingsSectionCommand(initial, 'voice', 4)).toBe(initial)
    expect(applySettingsSectionCommand(initial, 'adapters', 5)).toEqual({
      activeSection: 'adapters',
      appliedEpoch: 5,
    })
  })
})

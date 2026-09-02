import { describe, expect, it } from 'vitest'

import { titlebarPointerIntent } from './titlebarInteractions'

describe('titlebarPointerIntent', () => {
  it('starts dragging from a primary-button press on a passive titlebar surface', () => {
    expect(titlebarPointerIntent({ button: 0, clickCount: 1, interactive: false })).toBe('drag')
  })

  it('toggles maximization on the second primary-button press', () => {
    expect(titlebarPointerIntent({ button: 0, clickCount: 2, interactive: false })).toBe(
      'toggle-maximize',
    )
  })

  it('leaves tabs, buttons, and non-primary presses alone', () => {
    expect(titlebarPointerIntent({ button: 0, clickCount: 1, interactive: true })).toBe('ignore')
    expect(titlebarPointerIntent({ button: 0, clickCount: 2, interactive: true })).toBe('ignore')
    expect(titlebarPointerIntent({ button: 1, clickCount: 1, interactive: false })).toBe('ignore')
  })
})

import { describe, expect, it } from 'vitest'

import {
  comboFromEvent,
  comboParts,
  isAcceptableCombo,
  matchesShortcut,
} from './shortcutSettings'

function keyEvent(partial: {
  key: string
  code: string
  metaKey?: boolean
  ctrlKey?: boolean
  altKey?: boolean
  shiftKey?: boolean
  repeat?: boolean
}) {
  return {
    key: partial.key,
    code: partial.code,
    metaKey: partial.metaKey ?? false,
    ctrlKey: partial.ctrlKey ?? false,
    altKey: partial.altKey ?? false,
    shiftKey: partial.shiftKey ?? false,
    repeat: partial.repeat ?? false,
  } as unknown as KeyboardEvent
}

describe('comboParts', () => {
  it('splits a shortcut into display chips', () => {
    expect(comboParts('Ctrl+Shift+R')).toEqual(['Ctrl', 'Shift', 'R'])
    expect(comboParts('Cmd+Alt+1')).toEqual(['Cmd', 'Alt', '1'])
    expect(comboParts('F7')).toEqual(['F7'])
  })
})

describe('isAcceptableCombo', () => {
  it('rejects bare non-function keys', () => {
    expect(isAcceptableCombo('R')).toBe(false)
    expect(isAcceptableCombo('1')).toBe(false)
  })

  it('accepts function keys alone and any modified combo', () => {
    expect(isAcceptableCombo('F7')).toBe(true)
    expect(isAcceptableCombo('F24')).toBe(true)
    expect(isAcceptableCombo('Ctrl+R')).toBe(true)
    expect(isAcceptableCombo('Ctrl+Shift+1')).toBe(true)
  })

  it('rejects malformed values', () => {
    expect(isAcceptableCombo('')).toBe(false)
    expect(isAcceptableCombo('Ctrl+')).toBe(false)
  })
})

describe('comboFromEvent', () => {
  it('captures letters with modifiers', () => {
    expect(
      comboFromEvent(keyEvent({ key: 'R', code: 'KeyR', ctrlKey: true, shiftKey: true })),
    ).toBe('Ctrl+Shift+R')
  })

  it('uses the physical digit with shift held', () => {
    expect(
      comboFromEvent(keyEvent({ key: '!', code: 'Digit1', ctrlKey: true, shiftKey: true })),
    ).toBe('Ctrl+Shift+1')
  })

  it('maps cmd/meta to Cmd', () => {
    expect(
      comboFromEvent(keyEvent({ key: 'C', code: 'KeyC', metaKey: true })),
    ).toBe('Cmd+C')
  })

  it('captures named keys', () => {
    expect(
      comboFromEvent(keyEvent({ key: ' ', code: 'Space', ctrlKey: true })),
    ).toBe('Ctrl+Space')
    expect(
      comboFromEvent(keyEvent({ key: 'ArrowUp', code: 'ArrowUp', altKey: true })),
    ).toBe('Alt+ArrowUp')
  })

  it('returns null for modifier-only, Escape, and unsupported keys', () => {
    expect(comboFromEvent(keyEvent({ key: 'Control', code: 'ControlLeft' }))).toBeNull()
    expect(comboFromEvent(keyEvent({ key: 'Escape', code: 'Escape' }))).toBeNull()
    expect(
      comboFromEvent(keyEvent({ key: 'AudioVolumeMute', code: 'AudioVolumeMute', ctrlKey: true })),
    ).toBeNull()
  })
})

describe('matchesShortcut', () => {
  it('matches the exact combination', () => {
    const event = keyEvent({ key: 'R', code: 'KeyR', ctrlKey: true, shiftKey: true })
    expect(matchesShortcut(event, 'Ctrl+Shift+R')).toBe(true)
    expect(matchesShortcut(event, 'Ctrl+Shift+1')).toBe(false)
    expect(matchesShortcut(event, 'Cmd+Shift+R')).toBe(false)
  })
})

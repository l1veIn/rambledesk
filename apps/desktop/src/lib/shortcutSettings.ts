import { invoke } from '@tauri-apps/api/core'
import { writable } from 'svelte/store'

export type ShortcutAction = 'rambleToggle' | 'screenCapture'

export type ShortcutConfig = {
  rambleToggle: string
  screenCapture: string
}

const DEFAULT_SHORTCUTS: ShortcutConfig = {
  rambleToggle: 'Ctrl+Shift+R',
  screenCapture: 'Ctrl+1',
}

/** Mirrors the native `shortcuts.json` config; the native side is authoritative. */
export const shortcutSettings = writable<ShortcutConfig>(DEFAULT_SHORTCUTS)

export async function refreshShortcutSettings(): Promise<void> {
  try {
    shortcutSettings.set(await invoke<ShortcutConfig>('get_shortcut_settings'))
  } catch {
    // Keep the defaults; the native side logs the failure.
  }
}

export async function updateShortcut(
  action: ShortcutAction,
  shortcut: string,
): Promise<void> {
  shortcutSettings.set(
    await invoke<ShortcutConfig>('set_shortcut_setting', { action, shortcut }),
  )
}

export async function resetShortcuts(): Promise<void> {
  shortcutSettings.set(await invoke<ShortcutConfig>('reset_shortcut_settings'))
}

/**
 * While the settings dialog records a combination, the native handlers swallow
 * presses so typing the currently active shortcut does not trigger its action.
 * The same applies to the in-window keydown fallback (see
 * `isLocalCaptureActive`).
 */
export function setShortcutCaptureActive(active: boolean): Promise<void> {
  return invoke<void>('set_shortcut_capture_active', { active })
}

let localCaptureActive = false

/** Mirrors `setShortcutCaptureActive` for the in-window keydown fallback. */
export function setLocalCaptureActive(active: boolean): void {
  localCaptureActive = active
}

/** True while the settings dialog is recording a combination. */
export function isLocalCaptureActive(): boolean {
  return localCaptureActive
}

/** Splits "Ctrl+Shift+R" into chips for display. */
export function comboParts(combo: string): string[] {
  return combo
    .split('+')
    .map((part) => part.trim())
    .filter(Boolean)
}

/** True when the combo can be registered as a global shortcut. */
export function isAcceptableCombo(combo: string): boolean {
  const parts = comboParts(combo)
  if (parts.length === 0) return false
  if (parts.length === 1) return /^F(?:[1-9]|1[0-9]|2[0-4])$/i.test(parts[0] ?? '')
  return parts[0] !== undefined
}

/**
 * Converts a KeyboardEvent into the native shortcut string
 * ("Cmd+Ctrl+Alt+Shift+K"), or null for unsupported keys and modifier-only
 * presses. Validity (modifier requirement) is checked separately via
 * `isAcceptableCombo`.
 */
export function comboFromEvent(event: KeyboardEvent): string | null {
  if (['Control', 'Alt', 'Shift', 'Meta'].includes(event.key)) return null
  if (event.key === 'Escape') return null
  const key = keyToken(event)
  if (!key) return null
  const parts: string[] = []
  if (event.metaKey) parts.push('Cmd')
  if (event.ctrlKey) parts.push('Ctrl')
  if (event.altKey) parts.push('Alt')
  if (event.shiftKey) parts.push('Shift')
  parts.push(key)
  return parts.join('+')
}

/** True when the event matches the given shortcut string. */
export function matchesShortcut(event: KeyboardEvent, combo: string): boolean {
  const captured = comboFromEvent(event)
  return captured !== null && captured === combo
}

const PUNCTUATION: Record<string, string> = {
  Comma: ',',
  Period: '.',
  Slash: '/',
  Semicolon: ';',
  Quote: "'",
  Backquote: '`',
  Backslash: '\\',
  Minus: '-',
  BracketLeft: '[',
  BracketRight: ']',
  Equal: '=',
}

function keyToken(event: KeyboardEvent): string | null {
  const letter = /^Key([A-Z])$/.exec(event.code)
  if (letter) return letter[1] ?? null
  const digit = /^Digit([0-9])$/.exec(event.code)
  if (digit) return digit[1] ?? null
  const fn = /^F(?:([1-9])|(1[0-9])|(2[0-4]))$/.exec(event.code)
  if (fn) {
    const number = fn[1] ?? fn[2] ?? fn[3]
    return number ? `F${number}` : null
  }
  if (event.code === 'Space') return 'Space'
  if (event.code === 'Enter') return 'Enter'
  if (event.code === 'Tab') return 'Tab'
  if (event.code === 'Backspace') return 'Backspace'
  if (event.code === 'Delete') return 'Delete'
  if (event.code === 'Home') return 'Home'
  if (event.code === 'End') return 'End'
  if (event.code === 'PageUp') return 'PageUp'
  if (event.code === 'PageDown') return 'PageDown'
  if (event.code === 'ArrowUp') return 'ArrowUp'
  if (event.code === 'ArrowDown') return 'ArrowDown'
  if (event.code === 'ArrowLeft') return 'ArrowLeft'
  if (event.code === 'ArrowRight') return 'ArrowRight'
  return PUNCTUATION[event.code] ?? null
}

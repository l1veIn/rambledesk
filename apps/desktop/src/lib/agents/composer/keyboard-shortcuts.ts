// Adapted from xintaofei/codeg src/lib/keyboard-shortcuts.ts, commit 3ebdfed.
// SPDX-License-Identifier: Apache-2.0
// RambleDesk: only the composer's Enter-based bindings are accepted here.
import type { SubmitKeyEvent } from './submit-key'

export function matchShortcutEvent(event: SubmitKeyEvent, shortcut: string): boolean {
  const parts = shortcut.toLowerCase().split('+')
  if (parts.at(-1) !== 'enter' || event.key !== 'Enter') return false
  const modifiers = parts.slice(0, -1)
  if (modifiers.some((part) => !['mod', 'ctrl', 'meta', 'alt', 'shift'].includes(part))) return false
  const hasMod = modifiers.includes('mod')
  if (hasMod ? !(event.ctrlKey || event.metaKey) :
    event.ctrlKey !== modifiers.includes('ctrl') || event.metaKey !== modifiers.includes('meta')) return false
  return event.altKey === modifiers.includes('alt') && event.shiftKey === modifiers.includes('shift')
}

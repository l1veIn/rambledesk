import type { ITheme } from '@xterm/xterm'
import { writable } from 'svelte/store'

export type TerminalAppearanceId = 'midnight' | 'paper' | 'amber' | 'forest'
export type TerminalAppearance = { id: TerminalAppearanceId; label: string; muted: string; theme: ITheme }
export const TERMINAL_APPEARANCE_KEY = 'rambledesk.terminal.appearance.v1'
export const TERMINAL_APPEARANCES: readonly TerminalAppearance[] = [
  { id: 'midnight', label: 'Midnight', muted: '#9ca3af', theme: {
    background: '#16191f', foreground: '#e5e7eb', cursor: '#e5e7eb', selectionBackground: '#5873a966',
    black: '#20242c', red: '#e06c75', green: '#98c379', yellow: '#e5c07b',
    blue: '#61afef', magenta: '#c678dd', cyan: '#56b6c2', white: '#d1d5db',
    brightBlack: '#6b7280', brightRed: '#f28b94', brightGreen: '#b3d994', brightYellow: '#f5d89c',
    brightBlue: '#8fc8f5', brightMagenta: '#d9a0eb', brightCyan: '#83d0d9', brightWhite: '#ffffff',
  } },
  { id: 'paper', label: 'Paper', muted: '#6b7280', theme: {
    background: '#faf9f6', foreground: '#30343b', cursor: '#30343b', selectionBackground: '#8ab4e866',
    black: '#30343b', red: '#ad343b', green: '#347044', yellow: '#8a620d',
    blue: '#2e639f', magenta: '#88539a', cyan: '#247782', white: '#dddcd6',
    brightBlack: '#727782', brightRed: '#c0444b', brightGreen: '#3c7e50', brightYellow: '#987014',
    brightBlue: '#3872b4', brightMagenta: '#995cae', brightCyan: '#288591', brightWhite: '#ffffff',
  } },
  { id: 'amber', label: 'Amber', muted: '#b7a89a', theme: {
    background: '#251c19', foreground: '#f0dfc6', cursor: '#f5bd72', selectionBackground: '#b67c4766',
    black: '#332823', red: '#e78a7f', green: '#b3be83', yellow: '#e7bb78',
    blue: '#8aafc7', magenta: '#cda0bd', cyan: '#8ebfb2', white: '#e0ceba',
    brightBlack: '#9b8576', brightRed: '#f6a498', brightGreen: '#c8d299', brightYellow: '#f5cf98',
    brightBlue: '#a9c7dc', brightMagenta: '#e1b8d0', brightCyan: '#abd8c9', brightWhite: '#fff3df',
  } },
  { id: 'forest', label: 'Forest', muted: '#96afa2', theme: {
    background: '#14231e', foreground: '#dcebe1', cursor: '#b8d8bf', selectionBackground: '#67977c66',
    black: '#22352b', red: '#df8d8b', green: '#a7ce9c', yellow: '#d7c38c',
    blue: '#8eb9d5', magenta: '#c4a0ca', cyan: '#88c7bd', white: '#d0dfd4',
    brightBlack: '#789182', brightRed: '#f0aaa7', brightGreen: '#c0e2b7', brightYellow: '#eedaa7',
    brightBlue: '#add0e8', brightMagenta: '#dabade', brightCyan: '#a4dfd4', brightWhite: '#f3faf5',
  } },
]

export function terminalAppearanceFor(id: unknown): TerminalAppearance {
  return TERMINAL_APPEARANCES.find((appearance) => appearance.id === id) ?? TERMINAL_APPEARANCES[0]
}
function readAppearance(): TerminalAppearance {
  try { return terminalAppearanceFor(localStorage.getItem(TERMINAL_APPEARANCE_KEY)) }
  catch { return TERMINAL_APPEARANCES[0] }
}
const preferences = writable(readAppearance())
export const terminalAppearance = { subscribe: preferences.subscribe }

/** Cosmetic changes stay usable even when this browser cannot save preferences. */
export function setTerminalAppearance(id: TerminalAppearanceId): boolean {
  const next = terminalAppearanceFor(id)
  preferences.set(next)
  try { localStorage.setItem(TERMINAL_APPEARANCE_KEY, next.id); return true }
  catch { return false }
}

let references = 0
function onStorage(event: StorageEvent) {
  if (event.storageArea && event.storageArea !== localStorage) return
  if (event.key === TERMINAL_APPEARANCE_KEY || event.key === null) preferences.set(readAppearance())
}
export function initializeTerminalAppearance(): () => void {
  if (references++ === 0) {
    preferences.set(readAppearance())
    window.addEventListener('storage', onStorage)
  }
  let released = false
  return () => {
    if (released) return
    released = true
    if (--references === 0) window.removeEventListener('storage', onStorage)
  }
}

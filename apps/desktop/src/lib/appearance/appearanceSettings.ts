/**
 * Palette ids, swatches and zoom steps adapted from Codeg's src/lib/theme-presets.ts.
 * Copyright Codeg authors and contributors. SPDX-License-Identifier: Apache-2.0
 * Upstream: xintaofei/codeg, commit 3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1.
 * RambleDesk adds its existing palette, bounded settings and safe font resolution.
 */
export const PALETTES = [
  { id: 'classic', label: 'RambleDesk', swatch: '#2775ca' },
  { id: 'neutral', label: 'Neutral', swatch: 'oklch(0.205 0 0)' },
  { id: 'zinc', label: 'Zinc', swatch: 'oklch(0.21 0.006 285.885)' },
  { id: 'slate', label: 'Slate', swatch: 'oklch(0.208 0.042 265.755)' },
  { id: 'stone', label: 'Stone', swatch: 'oklch(0.216 0.006 56.043)' },
  { id: 'gray', label: 'Gray', swatch: 'oklch(0.21 0.034 264.665)' },
  { id: 'red', label: 'Red', swatch: 'oklch(0.637 0.237 25.331)' },
  { id: 'rose', label: 'Rose', swatch: 'oklch(0.645 0.246 16.439)' },
  { id: 'orange', label: 'Orange', swatch: 'oklch(0.705 0.213 47.604)' },
  { id: 'green', label: 'Green', swatch: 'oklch(0.723 0.219 149.579)' },
  { id: 'blue', label: 'Blue', swatch: 'oklch(0.546 0.245 262.881)' },
  { id: 'yellow', label: 'Yellow', swatch: 'oklch(0.795 0.184 86.047)' },
  { id: 'violet', label: 'Violet', swatch: 'oklch(0.606 0.25 292.717)' },
] as const

export type PaletteId = (typeof PALETTES)[number]['id']
export type FontId = 'system' | 'system-mono' | 'inter' | 'geist' | 'jetbrains-mono' | 'fira-code' | 'geist-mono' | 'custom'
type FontOption = Readonly<{ id: FontId; label: string; stack: string }>

const SYSTEM_UI = '"Segoe UI Variable", Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", "Microsoft YaHei UI", sans-serif'
const SYSTEM_MONO = 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace'

export const UI_FONTS = [
  { id: 'system', label: 'System', stack: SYSTEM_UI },
  { id: 'system-mono', label: 'System monospace', stack: SYSTEM_MONO },
  { id: 'inter', label: 'Inter', stack: `"Inter Variable", ${SYSTEM_UI}` },
  { id: 'geist', label: 'Geist', stack: `"Geist Variable", ${SYSTEM_UI}` },
  { id: 'jetbrains-mono', label: 'JetBrains Mono', stack: `"JetBrains Mono Variable", ${SYSTEM_MONO}` },
  { id: 'fira-code', label: 'Fira Code', stack: `"Fira Code Variable", ${SYSTEM_MONO}` },
  { id: 'geist-mono', label: 'Geist Mono', stack: `"Geist Mono Variable", ${SYSTEM_MONO}` },
  { id: 'custom', label: 'Custom', stack: SYSTEM_UI },
] as const satisfies readonly FontOption[]

export const CODE_FONTS = [
  { id: 'system-mono', label: 'System monospace', stack: SYSTEM_MONO },
  { id: 'jetbrains-mono', label: 'JetBrains Mono', stack: `"JetBrains Mono Variable", ${SYSTEM_MONO}` },
  { id: 'fira-code', label: 'Fira Code', stack: `"Fira Code Variable", ${SYSTEM_MONO}` },
  { id: 'geist-mono', label: 'Geist Mono', stack: `"Geist Mono Variable", ${SYSTEM_MONO}` },
  { id: 'custom', label: 'Custom', stack: SYSTEM_MONO },
] as const satisfies readonly FontOption[]

export const ZOOM_LEVELS = [80, 90, 100, 110, 125, 150, 175, 200, 250, 300] as const
export const CODE_FONT_SIZES = [10, 11, 12, 13, 14, 15, 16, 18, 20] as const

export type AppearanceSettings = {
  palette: PaletteId
  zoom: number
  uiFont: FontId
  uiCustomFont: string
  codeFont: FontId
  codeCustomFont: string
  codeFontSize: number
  background: 'pattern' | 'solid' | 'image'
  backgroundFit: 'cover' | 'contain' | 'center' | 'tile'
  backgroundMask: number
  backgroundBlur: number
  panelOpacity: number
}

export const DEFAULT_APPEARANCE: Readonly<AppearanceSettings> = Object.freeze({
  palette: 'classic', zoom: 100, uiFont: 'system', uiCustomFont: '',
  codeFont: 'system-mono', codeCustomFont: '', codeFontSize: 13,
  background: 'pattern', backgroundFit: 'cover', backgroundMask: 82,
  backgroundBlur: 0, panelOpacity: 30,
})

function choice<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  return typeof value === 'string' && allowed.includes(value as T) ? value as T : fallback
}

function bounded(value: unknown, minimum: number, maximum: number, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value)
    ? Math.max(minimum, Math.min(maximum, Math.round(value))) : fallback
}

function nearest(value: unknown, allowed: readonly number[], fallback: number): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return fallback
  return allowed.reduce((best, candidate) => Math.abs(candidate - value) < Math.abs(best - value) ? candidate : best)
}

/** Family names only: CSS expressions, escapes and declaration syntax are not preferences. */
function customFamilies(value: unknown): string[] | null {
  if (typeof value !== 'string' || value.length > 160 || /[\u0000-\u001f\u007f]/u.test(value)) return null
  const families = value.split(',').map(part => {
    const name = part.trim()
    const quoted = (name.startsWith('"') && name.endsWith('"')) || (name.startsWith("'") && name.endsWith("'"))
    return (quoted ? name.slice(1, -1) : name).trim()
  })
  return families.length <= 8 && families.every(name => name.length > 0 && /^[\p{L}\p{M}\p{N} ._'"-]+$/u.test(name))
    ? families : null
}

function customFont(value: unknown): string {
  return customFamilies(value) ? (value as string).trim() : ''
}

export function normalizeAppearance(value: unknown): AppearanceSettings {
  const input: Record<string, unknown> = value && typeof value === 'object' && !Array.isArray(value)
    ? value as Record<string, unknown> : {}
  return {
    palette: choice(input.palette, PALETTES.map(palette => palette.id), DEFAULT_APPEARANCE.palette),
    zoom: nearest(input.zoom, ZOOM_LEVELS, DEFAULT_APPEARANCE.zoom),
    uiFont: choice(input.uiFont, UI_FONTS.map(font => font.id), DEFAULT_APPEARANCE.uiFont),
    uiCustomFont: customFont(input.uiCustomFont),
    codeFont: choice(input.codeFont, CODE_FONTS.map(font => font.id), DEFAULT_APPEARANCE.codeFont),
    codeCustomFont: customFont(input.codeCustomFont),
    codeFontSize: nearest(input.codeFontSize, CODE_FONT_SIZES, DEFAULT_APPEARANCE.codeFontSize),
    background: choice(input.background, ['pattern', 'solid', 'image'], DEFAULT_APPEARANCE.background),
    backgroundFit: choice(input.backgroundFit, ['cover', 'contain', 'center', 'tile'], DEFAULT_APPEARANCE.backgroundFit),
    backgroundMask: bounded(input.backgroundMask, 0, 99, DEFAULT_APPEARANCE.backgroundMask),
    backgroundBlur: bounded(input.backgroundBlur, 0, 24, DEFAULT_APPEARANCE.backgroundBlur),
    panelOpacity: bounded(input.panelOpacity, 0, 100, DEFAULT_APPEARANCE.panelOpacity),
  }
}

export function resolveFontFamily(settings: AppearanceSettings, kind: 'ui' | 'code'): string {
  const fonts: readonly FontOption[] = kind === 'ui' ? UI_FONTS : CODE_FONTS
  const id = kind === 'ui' ? settings.uiFont : settings.codeFont
  const fallback = fonts[0].stack
  if (id !== 'custom') return fonts.find(font => font.id === id)?.stack ?? fallback
  const families = customFamilies(kind === 'ui' ? settings.uiCustomFont : settings.codeCustomFont)
  if (!families) return fallback
  return `${families.map(name => `"${name.replace(/"/g, '\\"')}"`).join(', ')}, ${fallback}`
}

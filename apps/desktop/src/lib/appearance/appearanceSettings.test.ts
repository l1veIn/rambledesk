import { describe, expect, it } from 'vitest'
import {
  CODE_FONTS, CODE_FONT_SIZES, DEFAULT_APPEARANCE, PALETTES, UI_FONTS, ZOOM_LEVELS,
  normalizeAppearance, resolveFontFamily,
} from './appearanceSettings'

describe('appearance settings', () => {
  it('preserves the existing RambleDesk appearance for missing or malformed storage', () => {
    for (const value of [undefined, null, [], 'dark', 13, true]) {
      expect(normalizeAppearance(value)).toEqual(DEFAULT_APPEARANCE)
    }
    expect(DEFAULT_APPEARANCE).toEqual({
      palette: 'classic', zoom: 100, uiFont: 'system', uiCustomFont: '',
      codeFont: 'system-mono', codeCustomFont: '', codeFontSize: 13,
      background: 'pattern', backgroundFit: 'cover', backgroundMask: 82,
      backgroundBlur: 0, panelOpacity: 30,
    })
    expect(normalizeAppearance({})).not.toBe(DEFAULT_APPEARANCE)
  })

  it('retains valid independent preferences and ignores unknown fields', () => {
    const value = { ...DEFAULT_APPEARANCE, palette: 'violet', zoom: 175, uiFont: 'geist',
      codeFont: 'jetbrains-mono', codeFontSize: 16, background: 'image',
      backgroundFit: 'tile', backgroundMask: 50, backgroundBlur: 12, panelOpacity: 70 }
    expect(normalizeAppearance({ ...value, image: 'private.png' })).toEqual(value)
    expect(normalizeAppearance(normalizeAppearance(value))).toEqual(value)
  })

  it('keeps font choices scoped and rejects unknown enum values', () => {
    expect(normalizeAppearance({ palette: 'unknown', uiFont: 'unknown', codeFont: 'inter',
      background: 'url(image)', backgroundFit: 'stretch' })).toEqual(DEFAULT_APPEARANCE)
    expect(UI_FONTS.map(font => font.id)).toEqual(['system', 'system-mono', 'inter', 'geist', 'jetbrains-mono', 'fira-code', 'geist-mono', 'custom'])
    expect(CODE_FONTS.map(font => font.id)).toEqual(['system-mono', 'jetbrains-mono', 'fira-code', 'geist-mono', 'custom'])
  })

  it('bounds numeric controls and restores defaults for nonnumeric values', () => {
    expect(normalizeAppearance({ zoom: 999, codeFontSize: 99, backgroundMask: -10,
      backgroundBlur: 50, panelOpacity: 101 })).toMatchObject({
      zoom: 300, codeFontSize: 20, backgroundMask: 0, backgroundBlur: 24, panelOpacity: 100,
    })
    expect(normalizeAppearance({ zoom: -1, codeFontSize: 2, backgroundMask: 42.8,
      backgroundBlur: 2.2, panelOpacity: -1 })).toMatchObject({
      zoom: 80, codeFontSize: 10, backgroundMask: 43, backgroundBlur: 2, panelOpacity: 0,
    })
    expect(normalizeAppearance({ zoom: 124, codeFontSize: 14.8 })).toMatchObject({ zoom: 125, codeFontSize: 15 })
    for (const value of [NaN, Infinity, -Infinity, '150', null, {}, true]) {
      expect(normalizeAppearance({ zoom: value, codeFontSize: value, backgroundMask: value,
        backgroundBlur: value, panelOpacity: value })).toEqual(DEFAULT_APPEARANCE)
    }
    expect(ZOOM_LEVELS).toEqual([80, 90, 100, 110, 125, 150, 175, 200, 250, 300])
    expect(CODE_FONT_SIZES).toEqual([10, 11, 12, 13, 14, 15, 16, 18, 20])
    expect(normalizeAppearance({ backgroundMask: 100, codeFontSize: 17 })).toMatchObject({ backgroundMask: 99, codeFontSize: 16 })
  })

  it('quotes custom family names and preserves system fallbacks', () => {
    const settings = normalizeAppearance({ uiFont: 'custom', uiCustomFont: '  "Noto Sans SC", 思源黑体  ',
      codeFont: 'custom', codeCustomFont: 'Acme "Display", Sam\'s Mono' })
    expect(resolveFontFamily(settings, 'ui')).toBe(`"Noto Sans SC", "思源黑体", ${UI_FONTS[0].stack}`)
    expect(resolveFontFamily(settings, 'code')).toBe(`"Acme \\"Display\\"", "Sam's Mono", ${CODE_FONTS[0].stack}`)
    expect(resolveFontFamily({ ...DEFAULT_APPEARANCE, uiFont: 'inter' }, 'ui')).toBe(UI_FONTS.find(font => font.id === 'inter')?.stack)
    expect(resolveFontFamily({ ...DEFAULT_APPEARANCE, uiFont: 'inter' }, 'ui')).toContain('"Inter Variable"')
  })

  it('rejects CSS expressions, control characters, oversized input, and empty family lists', () => {
    for (const family of ['Arial; color:red', 'url(https://example.test/font)', 'var(--private)',
      'Arial}body{display:none', 'Arial\\22', 'Arial\nGeorgia', '<style>', 'Arial,,Georgia',
      ',Arial', '""', 'a'.repeat(161)]) {
      const settings = normalizeAppearance({ uiFont: 'custom', uiCustomFont: family,
        codeFont: 'custom', codeCustomFont: family })
      expect(settings.uiCustomFont).toBe('')
      expect(settings.codeCustomFont).toBe('')
      expect(resolveFontFamily(settings, 'ui')).toBe(UI_FONTS[0].stack)
      expect(resolveFontFamily(settings, 'code')).toBe(CODE_FONTS[0].stack)
    }
  })

  it('offers unique complete palette identities and fixed safe swatches', () => {
    expect(PALETTES.map(palette => palette.id)).toEqual([
      'classic', 'neutral', 'zinc', 'slate', 'stone', 'gray', 'red', 'rose',
      'orange', 'green', 'blue', 'yellow', 'violet',
    ])
    for (const palette of PALETTES) {
      expect(palette.label.length).toBeGreaterThan(0)
      expect(palette.swatch).toMatch(/^(#[0-9a-f]{6}|oklch\([\d.]+ [\d.]+ [\d.]+\))$/)
      expect(normalizeAppearance({ palette: palette.id }).palette).toBe(palette.id)
    }
  })
})

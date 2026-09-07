import { describe, expect, it } from 'vitest'
import { PALETTES } from './appearanceSettings'

// Vitest disables CSS imports; read the real stylesheet in its Node test runtime.
const filesystemModule = 'node:fs'
const { readFileSync } = await import(/* @vite-ignore */ filesystemModule) as {
  readFileSync(path: URL, encoding: 'utf8'): string
}

function palettes() {
  const css = readFileSync(new URL('./palettes.css', import.meta.url), 'utf8')
  return new Map([...css.matchAll(/:root\[data-palette="([a-z]+)"\]\[data-theme="(light|dark)"\]\s*\{([^}]+)\}/g)]
    .map(([, id, theme, body]) => [`${id}:${theme}`, Object.fromEntries(
      [...body.matchAll(/--([a-z0-9-]+):\s*([^;]+);/g)].map(([, token, value]) => [token, value.trim()]),
    )]))
}

/** WCAG luminance after converting the actual OKLCH palette values into linear sRGB. */
function luminance(value: string): number {
  if (/^#[0-9a-f]{6}$/i.test(value)) {
    const channels = [1, 3, 5].map(start => Number.parseInt(value.slice(start, start + 2), 16) / 255)
      .map(channel => channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4)
    return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722
  }
  const match = /^oklch\(([\d.]+) ([\d.]+) ([\d.]+)\)$/.exec(value)
  if (!match) throw new Error(`Unsupported contrast sample: ${value}`)
  const [lightness, chroma, hue] = match.slice(1).map(Number)
  const a = chroma * Math.cos(hue * Math.PI / 180)
  const b = chroma * Math.sin(hue * Math.PI / 180)
  const l = (lightness + 0.3963377774 * a + 0.2158037573 * b) ** 3
  const m = (lightness - 0.1055613458 * a - 0.0638541728 * b) ** 3
  const s = (lightness - 0.0894841775 * a - 1.291485548 * b) ** 3
  const channels = [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s].map(channel => Math.max(0, Math.min(1, channel)))
  return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722
}

function contrast(first: string, second: string) {
  const a = luminance(first)
  const b = luminance(second)
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05)
}

describe('complete appearance palettes', () => {
  it('defines every surface, text, chrome, and status token in both modes', () => {
    const definitions = palettes()
    expect(definitions.size).toBe(PALETTES.length * 2)
    for (const { id } of PALETTES) for (const theme of ['light', 'dark']) {
      const tokens = definitions.get(`${id}:${theme}`)!
      for (const name of ['background', 'foreground', 'card', 'card-foreground', 'popover',
        'popover-foreground', 'primary', 'primary-foreground', 'secondary', 'secondary-foreground',
        'muted', 'muted-foreground', 'accent', 'accent-foreground', 'destructive', 'border', 'input',
        'ring', 'sidebar', 'sidebar-foreground', 'sidebar-primary', 'sidebar-primary-foreground',
        'sidebar-accent', 'sidebar-accent-foreground', 'sidebar-border', 'sidebar-ring',
        'titlebar-background', 'app-background', 'glass', 'shadow', 'success', 'success-foreground',
        'warning', 'warning-foreground', 'info']) {
        expect(tokens[name], `${id}:${theme} --${name}`).toBeTruthy()
      }
    }
  })

  it('keeps the classic RambleDesk colors and fixed palette swatches', () => {
    const definitions = palettes()
    expect(definitions.get('classic:light')).toMatchObject({ background: '#f7f9fc', foreground: '#20334b', primary: '#2775ca', sidebar: '#f7f9fc', 'titlebar-background': '#dfe7f0' })
    expect(definitions.get('classic:dark')).toMatchObject({ background: '#141f2c', foreground: '#dce8f5', primary: '#68a9ec', sidebar: '#162230', 'titlebar-background': '#0d1722' })
    for (const palette of PALETTES) expect(definitions.get(`${palette.id}:light`)?.primary).toBe(palette.swatch)
  })

  it('keeps body text and action labels readable in every light and dark palette', () => {
    for (const [key, tokens] of palettes()) {
      for (const [surface, text] of [['background', 'foreground'], ['card', 'card-foreground'],
        ['popover', 'popover-foreground'], ['sidebar', 'sidebar-foreground']]) {
        expect(contrast(tokens[surface], tokens[text]), `${key} ${surface}/${text}`).toBeGreaterThanOrEqual(7)
      }
      for (const [surface, text] of [['primary', 'primary-foreground'], ['sidebar-primary', 'sidebar-primary-foreground'],
        ['secondary', 'secondary-foreground'], ['accent', 'accent-foreground']]) {
        expect(contrast(tokens[surface], tokens[text]), `${key} ${surface}/${text}`).toBeGreaterThanOrEqual(4.5)
      }
    }
  })
})

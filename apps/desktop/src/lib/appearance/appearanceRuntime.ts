import { get, writable } from 'svelte/store'
import { createBrowserZoomCapability } from '../capabilities/browser/browserZoomCapability'
import { appearancePreferences, initializeAppearancePreferences, updateAppearance, workspaceBackground } from './appearancePreferences'
import { resolveFontFamily, ZOOM_LEVELS, type AppearanceSettings } from './appearanceSettings'

export const appearanceRuntimeError = writable<string | null>(null)

export function applyAppearanceStyles(root: HTMLElement, settings: AppearanceSettings): void {
  root.dataset.appearance = 'enabled'
  root.dataset.palette = settings.palette
  root.dataset.uiFont = settings.uiFont
  root.dataset.workspaceBackground = settings.background
  root.dataset.backgroundFit = settings.backgroundFit
  root.style.setProperty('--appearance-ui-font', resolveFontFamily(settings, 'ui'))
  root.style.setProperty('--appearance-code-font', resolveFontFamily(settings, 'code'))
  root.style.setProperty('--appearance-code-size', `${settings.codeFontSize}px`)
  root.style.setProperty('--workspace-background-mask', `${settings.backgroundMask}%`)
  root.style.setProperty('--workspace-background-blur', `${settings.backgroundBlur}px`)
  root.style.setProperty('--workspace-panel-opacity', `${settings.panelOpacity}%`)
  root.style.setProperty('--workspace-background-size', settings.backgroundFit === 'cover' || settings.backgroundFit === 'contain' ? settings.backgroundFit : 'auto')
  root.style.setProperty('--workspace-background-repeat', settings.backgroundFit === 'tile' ? 'repeat' : 'no-repeat')
}

/**
 * Palette and font tokens for isolated windows (speech overlay, ramble console).
 * Does not apply zoom or workspace wallpaper: those windows size themselves in
 * physical pixels and sit above other apps.
 */
export function initializeAppearanceStyles(
  root: HTMLElement = document.documentElement,
  onSettings?: (settings: AppearanceSettings) => void,
): () => void {
  const releasePreferences = initializeAppearancePreferences()
  const unsubscribe = appearancePreferences.subscribe(settings => {
    applyAppearanceStyles(root, settings)
    onSettings?.(settings)
  })
  return () => {
    unsubscribe()
    releasePreferences()
  }
}

/** Main workbench only: styles plus zoom and wallpaper. Overlay windows use initializeAppearanceStyles. */
export function initializeAppearance(options: { setZoom?: (factor: number) => Promise<void> } = {}): () => void {
  const root = document.documentElement
  const browserZoom = options.setZoom ? null : createBrowserZoomCapability(root)
  const setZoom = options.setZoom ?? browserZoom!.setZoom
  let active = true
  let appliedZoom: number | null = null
  let desiredZoom = get(appearancePreferences).zoom
  let applying = false

  async function applyZoom() {
    if (applying) return
    applying = true
    try {
      while (active && desiredZoom !== appliedZoom) {
        const next = desiredZoom
        try {
          await setZoom(next / 100)
          appliedZoom = next
          if (active) appearanceRuntimeError.set(null)
        } catch (cause) {
          if (active) appearanceRuntimeError.set(cause instanceof Error ? cause.message : String(cause))
          if (desiredZoom === next) break
        }
      }
    } finally {
      applying = false
      if (active && desiredZoom === appliedZoom) appearanceRuntimeError.set(null)
    }
  }

  const releaseStyles = initializeAppearanceStyles(root, settings => {
    desiredZoom = settings.zoom
    void applyZoom()
  })
  const unsubscribeBackground = workspaceBackground.subscribe(background => {
    root.dataset.backgroundReady = background.url ? 'true' : 'false'
    // Only object URLs created by our local Blob repository reach this variable.
    if (background.url) root.style.setProperty('--workspace-background-image', `url("${background.url}")`)
    else root.style.removeProperty('--workspace-background-image')
  })

  function onKeyDown(event: KeyboardEvent) {
    if (!(event.ctrlKey || event.metaKey) || event.altKey || event.isComposing) return
    const key = event.key
    if (!['+', '=', '-', '0'].includes(key)) return
    event.preventDefault()
    const index = ZOOM_LEVELS.indexOf(get(appearancePreferences).zoom as (typeof ZOOM_LEVELS)[number])
    const zoom = key === '0' ? 100 : ZOOM_LEVELS[Math.max(0, Math.min(ZOOM_LEVELS.length - 1, index + (key === '-' ? -1 : 1)))]
    try { updateAppearance({ zoom }) }
    catch (cause) { appearanceRuntimeError.set(cause instanceof Error ? cause.message : String(cause)) }
  }
  window.addEventListener('keydown', onKeyDown)
  return () => {
    if (!active) return
    active = false
    unsubscribeBackground()
    releaseStyles()
    window.removeEventListener('keydown', onKeyDown)
    if (browserZoom) void browserZoom.setZoom(1)
  }
}

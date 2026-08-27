import { mount } from 'svelte'

import { initializePreferences } from './lib/preferences'
import './app.css'

function reportFrontendError(context: string, message: string) {
  if (!('__TAURI_INTERNALS__' in window)) return
  void import('@tauri-apps/api/core')
    .then(({ invoke }) => invoke('log_frontend_error', { context, message }))
    .catch(() => undefined)
}

function configureContextMenuAndDevtools() {
  const isTauri = '__TAURI_INTERNALS__' in window
  if (!import.meta.env.DEV) {
    window.addEventListener(
      'contextmenu',
      (event) => {
        // The task brief must stay copyable: keep the native menu inside the
        // brief panel/preview, and anywhere the user already holds a text
        // selection, so right-click Copy works in the packaged app. Everywhere
        // else the native menu is suppressed because the app draws its own
        // chrome and menus.
        const target = event.target as Element | null
        const inTaskBrief =
          target?.closest('.task-brief, .task-brief-preview-content') ?? null
        const hasSelection = (window.getSelection()?.toString().length ?? 0) > 0
        if (inTaskBrief || hasSelection) return
        event.preventDefault()
      },
      { capture: true },
    )
    return
  }

  if (!isTauri) return
  window.addEventListener('keydown', (event) => {
    const key = event.key.toLowerCase()
    const inspectorShortcut =
      event.key === 'F12' ||
      (key === 'i' &&
        ((event.ctrlKey && event.shiftKey && !event.metaKey) ||
          (event.metaKey && event.altKey)))
    if (!inspectorShortcut) return
    event.preventDefault()
    void import('@tauri-apps/api/core')
      .then(({ invoke }) => invoke('open_main_devtools'))
      .catch((cause) => console.warn('Could not open DevTools', cause))
  })
}

function configureConsoleWarnings() {
  // Multiple TipTap editors each register the same linkify protocols; the
  // second registration warns "already initialized" by design and is harmless.
  // Keep real warnings flowing.
  const originalWarn = console.warn
  console.warn = (...args: unknown[]) => {
    const text = args.map(String).join(' ')
    if (text.includes('linkifyjs: already initialized')) return
    originalWarn(...args)
  }
}

window.addEventListener('error', (event) => {
  reportFrontendError('window', event.message || 'unknown window error')
})

window.addEventListener('unhandledrejection', (event) => {
  const reason = event.reason
  reportFrontendError(
    'unhandledrejection',
    reason instanceof Error ? `${reason.name}: ${reason.message}` : String(reason),
  )
})

initializePreferences()
configureContextMenuAndDevtools()
configureConsoleWarnings()

const captureMode = window.location.hash === '#capture'
const scrollCaptureMode = window.location.hash === '#capture-scroll'
const pinnedCaptureMode = window.location.hash.startsWith('#capture-pin=')
const rambleConsoleMode =
  window.location.hash === '#ramble-console' ||
  window.location.pathname.endsWith('/ramble-console')
if (captureMode || scrollCaptureMode || pinnedCaptureMode) {
  document.body.classList.add('capture-mode')
} else if (rambleConsoleMode) {
  document.body.classList.add('ramble-console-mode')
} else {
  document.body.classList.add('app-mode')
}

const target = document.getElementById('app')!

if (captureMode) {
  await import('./lib/screen-capture/screenshot-overlay.css')
  const { default: ScreenshotOverlay } = await import('./ScreenshotOverlay.svelte')
  mount(ScreenshotOverlay, { target })
} else if (scrollCaptureMode) {
  await import('./lib/screen-capture/screenshot-overlay.css')
  const { default: ScrollCaptureController } = await import('./ScrollCaptureController.svelte')
  mount(ScrollCaptureController, { target })
} else if (pinnedCaptureMode) {
  await import('./lib/screen-capture/screenshot-overlay.css')
  const { default: PinnedCapture } = await import('./PinnedCapture.svelte')
  mount(PinnedCapture, { target })
} else if (rambleConsoleMode) {
  await import('./lib/ramble-console.css')
  const { default: RambleConsole } = await import('./RambleConsole.svelte')
  mount(RambleConsole, { target })
} else {
  const { default: App } = await import('./App.svelte')
  mount(App, { target })
}

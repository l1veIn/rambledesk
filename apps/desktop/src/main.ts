import { mount } from 'svelte'

import { initializePreferences } from './lib/preferences'
import './app.css'

function reportFrontendError(context: string, message: string) {
  if (!('__TAURI_INTERNALS__' in window)) return
  void import('@tauri-apps/api/core')
    .then(({ invoke }) => invoke('log_frontend_error', { context, message }))
    .catch(() => undefined)
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

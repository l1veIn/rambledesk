import { mount } from 'svelte'

import App from './App.svelte'
import PinnedCapture from './PinnedCapture.svelte'
import RambleConsole from './RambleConsole.svelte'
import ScrollCaptureController from './ScrollCaptureController.svelte'
import ScreenshotOverlay from './ScreenshotOverlay.svelte'
import { initializePreferences } from './lib/preferences'
import './app.css'
import './lib/ramble-console.css'
import './lib/screen-capture/screenshot-overlay.css'

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

mount(
  captureMode
    ? ScreenshotOverlay
    : scrollCaptureMode
      ? ScrollCaptureController
      : pinnedCaptureMode
        ? PinnedCapture
        : rambleConsoleMode
          ? RambleConsole
          : App,
  { target: document.getElementById('app')! },
)

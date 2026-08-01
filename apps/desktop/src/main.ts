import { mount } from 'svelte'

import App from './App.svelte'
import PinnedCapture from './PinnedCapture.svelte'
import RambleConsole from './RambleConsole.svelte'
import ScrollCaptureController from './ScrollCaptureController.svelte'
import ScreenshotOverlay from './ScreenshotOverlay.svelte'
import { initializePreferences } from './lib/preferences'
import './app.css'

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

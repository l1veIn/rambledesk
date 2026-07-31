import { mount } from 'svelte'

import App from './App.svelte'
import RambleConsole from './RambleConsole.svelte'
import ScreenshotOverlay from './ScreenshotOverlay.svelte'
import { initializePreferences } from './lib/preferences'
import './app.css'

initializePreferences()

const captureMode = window.location.hash === '#capture'
const rambleConsoleMode =
  window.location.hash === '#ramble-console' ||
  window.location.pathname.endsWith('/ramble-console')
if (captureMode) document.body.classList.add('capture-mode')
if (rambleConsoleMode) document.body.classList.add('ramble-console-mode')

mount(captureMode ? ScreenshotOverlay : rambleConsoleMode ? RambleConsole : App, {
  target: document.getElementById('app')!,
})

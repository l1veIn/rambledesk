import { mount } from 'svelte'

import App from './App.svelte'
import ScreenshotOverlay from './ScreenshotOverlay.svelte'
import './app.css'

const captureMode = window.location.hash === '#capture'
if (captureMode) document.body.classList.add('capture-mode')

mount(captureMode ? ScreenshotOverlay : App, {
  target: document.getElementById('app')!,
})

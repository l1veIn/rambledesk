import { mount } from 'svelte'
import '../app.css'
import WorkbenchPreview from './WorkbenchPreview.svelte'
import { initializePreferences } from '$lib/preferences'

if (import.meta.env.DEV) {
  document.body.classList.add('app-mode', 'web-mode')
  initializePreferences()
  mount(WorkbenchPreview, { target: document.getElementById('app')! })
}

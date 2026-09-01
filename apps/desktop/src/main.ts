import { mount } from 'svelte'

import { initializePreferences } from './lib/preferences'
import { createWorkbenchComposition } from './lib/application/workbenchComposition'
import { selectWorkbenchEntry } from './lib/workbenchEntry'
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
        // brief, and anywhere the user already holds a text
        // selection, so right-click Copy works in the packaged app. Everywhere
        // else the native menu is suppressed because the app draws its own
        // chrome and menus.
        const target = event.target as Element | null
        const inTaskBrief = target?.closest('.task-brief') ?? null
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

const isTauri = '__TAURI_INTERNALS__' in window
const previewMode =
  import.meta.env.DEV &&
  !isTauri &&
  new URLSearchParams(window.location.search).get('preview') === 'fixtures'
const entry = selectWorkbenchEntry({
  isTauri,
  previewMode,
  pathname: window.location.pathname,
  hash: window.location.hash,
})

if (entry === 'capture' || entry === 'scroll-capture' || entry === 'pinned-capture') {
  document.body.classList.add('capture-mode')
} else if (entry === 'ramble-console') {
  document.body.classList.add('ramble-console-mode')
} else {
  document.body.classList.add('app-mode')
}

const target = document.getElementById('app')!

if (entry === 'browser') {
  const { default: BrowserWorkbenchRoot } = await import('./BrowserWorkbenchRoot.svelte')
  mount(BrowserWorkbenchRoot, { target })
} else if (entry === 'capture') {
  await import('./lib/screen-capture/screenshot-overlay.css')
  const { default: ScreenshotOverlay } = await import('./ScreenshotOverlay.svelte')
  mount(ScreenshotOverlay, { target })
} else if (entry === 'scroll-capture') {
  await import('./lib/screen-capture/screenshot-overlay.css')
  const { default: ScrollCaptureController } = await import('./ScrollCaptureController.svelte')
  mount(ScrollCaptureController, { target })
} else if (entry === 'pinned-capture') {
  await import('./lib/screen-capture/screenshot-overlay.css')
  const { default: PinnedCapture } = await import('./PinnedCapture.svelte')
  mount(PinnedCapture, { target })
} else if (entry === 'ramble-console') {
  await import('./lib/ramble-console.css')
  const { default: RambleConsole } = await import('./RambleConsole.svelte')
  mount(RambleConsole, { target })
} else {
  const { default: App } = await import('./App.svelte')
  const desktopTransport = isTauri
    ? new (await import('./lib/application/tauriApplicationTransport')).TauriApplicationTransport()
    : undefined
  const composition = createWorkbenchComposition({
    environment: isTauri ? 'desktop' : 'browser',
    previewMode,
    desktopTransport,
  })
  const publishedFeedbackAction = isTauri
    ? {
        label: 'Open feedback package' as const,
        async run(requestId: string) {
          const { invoke } = await import('@tauri-apps/api/core')
          await invoke('reveal_feedback_package', { input: { request_id: requestId } })
        },
      }
    : (await import('./lib/publishedFeedbackAction')).createBrowserPublishedFeedbackAction(
        composition.applicationTransport,
      )
  mount(App, { target, props: { ...composition, publishedFeedbackAction } })
}

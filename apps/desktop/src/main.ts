import { selectWorkbenchEntry } from './lib/workbenchEntry'
import { configureClientDiagnostics, diagnosticErrorCategory, recordClientDiagnostic, startClientDiagnostic } from './lib/diagnostics/clientDiagnostics'
import { runFrontendBootstrap, showStartupFailure } from './lib/startupFallback'
import './app.css'

if ('__TAURI_INTERNALS__' in window) {
  const instrumentation = import('./lib/desktop-shell/instrumentation')
  configureClientDiagnostics(event => instrumentation.then(module => module.TAURI_DESKTOP_SHELL_INSTRUMENTATION.recordClientDiagnostic(event)))
}

function frontendErrorCategory(cause: unknown): string {
  if (cause instanceof TypeError) return 'type_error'
  if (cause instanceof ReferenceError) return 'reference_error'
  if (cause instanceof RangeError) return 'range_error'
  if (cause instanceof SyntaxError) return 'syntax_error'
  return diagnosticErrorCategory(cause)
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
    void import('./lib/desktop-shell/instrumentation')
      .then(({ TAURI_DESKTOP_SHELL_INSTRUMENTATION }) =>
        TAURI_DESKTOP_SHELL_INSTRUMENTATION.openMainDevtools(),
      )
      .catch((cause) => console.warn('Could not open DevTools', cause))
  })
}

window.addEventListener('error', (event) => {
  recordClientDiagnostic({ activity: 'frontend_error', outcome: 'failed', details: {
    source: 'main', action: 'window', error_category: frontendErrorCategory(event.error), line: event.lineno, column: event.colno,
  } })
})

window.addEventListener('unhandledrejection', (event) => {
  recordClientDiagnostic({ activity: 'frontend_error', outcome: 'failed', details: {
    source: 'main', action: 'unhandledrejection', error_category: frontendErrorCategory(event.reason),
  } })
})

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
} else if (entry === 'ramble-console' || entry === 'speech-overlay') {
  document.body.classList.add('ramble-console-mode')
} else {
  document.body.classList.add('app-mode')
}

const target = document.getElementById('app')!

const finishStartup = startClientDiagnostic('application_startup', { source: entry === 'desktop' ? 'main' : entry })
await runFrontendBootstrap(async (signal) => {
  // Imports can fail before a Svelte component exists (including preferences
  // accessing unavailable browser storage). Keep them inside the recovery guard.
  const { mount } = await import('svelte')
  const { initializePreferences } = await import('./lib/preferences')
  signal.throwIfAborted()
  initializePreferences()
  configureContextMenuAndDevtools()
if (entry === 'browser') {
  const { default: BrowserWorkbenchRoot } = await import('./BrowserWorkbenchRoot.svelte')
  signal.throwIfAborted()
  mount(BrowserWorkbenchRoot, { target })
} else if (entry === 'capture') {
  await import('./lib/screen-capture/screenshot-overlay.css')
  const { default: ScreenshotOverlay } = await import('./ScreenshotOverlay.svelte')
  signal.throwIfAborted()
  mount(ScreenshotOverlay, { target })
} else if (entry === 'scroll-capture') {
  await import('./lib/screen-capture/screenshot-overlay.css')
  const { default: ScrollCaptureController } = await import('./ScrollCaptureController.svelte')
  signal.throwIfAborted()
  mount(ScrollCaptureController, { target })
} else if (entry === 'pinned-capture') {
  await import('./lib/screen-capture/screenshot-overlay.css')
  const { default: PinnedCapture } = await import('./PinnedCapture.svelte')
  signal.throwIfAborted()
  mount(PinnedCapture, { target })
} else if (entry === 'ramble-console') {
  await import('./lib/ramble-console.css')
  const { default: RambleConsole } = await import('./RambleConsole.svelte')
  signal.throwIfAborted()
  mount(RambleConsole, { target })
} else if (entry === 'speech-overlay') {
  const { default: SpeechOverlay } = await import('./SpeechOverlay.svelte')
  signal.throwIfAborted()
  mount(SpeechOverlay, { target })
} else {
  const { createWorkbenchComposition } = await import('./lib/application/workbenchComposition')
  const { default: App } = await import('./App.svelte')
  const tauriCapabilities = isTauri
    ? await import('./lib/capabilities/tauri')
    : undefined
  const capabilities = tauriCapabilities
    ? tauriCapabilities.createTauriWorkbenchCapabilities()
    : (await import('./lib/capabilities/unavailableCapabilities'))
        .createUnavailableWorkbenchCapabilities()
  const desktopTransport = isTauri
    ? new (await import('./lib/application/tauriApplicationTransport')).TauriApplicationTransport(
        capabilities.manifest,
      )
    : undefined
  const composition = createWorkbenchComposition({
    environment: isTauri ? 'desktop' : 'browser',
    previewMode,
    desktopTransport,
    capabilities,
  })
  const publishedFeedbackAction = tauriCapabilities
    ? tauriCapabilities.createTauriPublishedFeedbackAction()
    : (await import('./lib/publishedFeedbackAction')).createBrowserPublishedFeedbackAction(
        composition.applicationTransport,
      )
  signal.throwIfAborted()
  mount(App, { target, props: { ...composition, publishedFeedbackAction } })
}
  finishStartup('ok')
}, (failure) => {
  finishStartup('failed', { error_category: failure.category })
  showStartupFailure(target, failure)
})

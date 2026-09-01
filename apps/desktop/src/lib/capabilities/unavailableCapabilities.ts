import type { CapabilityName, CapabilityStatus } from './capabilityManifest'
import type { CapturePlugins } from './capturePlugin'
import {
  CapabilityUnavailableError,
  createWorkbenchCapabilities,
  type CapabilityErrorHandler,
  type CapabilitySlot,
  type CapabilityUnsubscribe,
  type WorkbenchCapabilities,
} from './workbenchCapabilities'

const UNSUPPORTED: CapabilityStatus = Object.freeze({
  availability: 'unavailable',
  source: 'none',
  reason: 'unsupported_environment',
})

function rejected(capability: CapabilityName): Promise<never> {
  return Promise.reject(new CapabilityUnavailableError(capability, UNSUPPORTED))
}

function unavailableSubscription(
  capability: CapabilityName,
  onError: CapabilityErrorHandler,
): CapabilityUnsubscribe {
  let active = true
  queueMicrotask(() => {
    if (active) onError(new CapabilityUnavailableError(capability, UNSUPPORTED))
  })
  return () => {
    active = false
  }
}

function slot<Implementation>(implementation: Implementation): CapabilitySlot<Implementation> {
  return Object.freeze({ status: UNSUPPORTED, implementation })
}

const UNAVAILABLE_SCREEN_CAPTURE = {
  onCandidate: (_handler, onError) => unavailableSubscription('screenCapture', onError),
  onFinished: (_handler, onError) => unavailableSubscription('screenCapture', onError),
  onShortcut: (_handler, onError) => unavailableSubscription('screenCapture', onError),
  begin: () => rejected('screenCapture'),
} satisfies CapturePlugins['screenCapture']

const UNAVAILABLE_CLIPBOARD_CAPTURE = {
  captureOnce: () => rejected('clipboardCapture'),
} satisfies CapturePlugins['clipboardCapture']

const UNAVAILABLE_IMAGE_PASTE = {
  subscribe: (_target, _handler, onError) =>
    unavailableSubscription('imagePaste', onError),
} satisfies CapturePlugins['imagePaste']

const UNAVAILABLE_CAPTURE_PLUGINS: CapturePlugins = Object.freeze({
  screenCapture: Object.freeze(UNAVAILABLE_SCREEN_CAPTURE),
  clipboardCapture: Object.freeze(UNAVAILABLE_CLIPBOARD_CAPTURE),
  imagePaste: Object.freeze(UNAVAILABLE_IMAGE_PASTE),
})

const UNAVAILABLE_WORKBENCH_CAPABILITIES = createWorkbenchCapabilities({
  windowControls: slot({
    platform: () => 'unknown' as const,
    isMaximized: () => rejected('windowControls'),
    minimize: () => rejected('windowControls'),
    toggleMaximize: () => rejected('windowControls'),
    close: () => rejected('windowControls'),
    startDragging: () => rejected('windowControls'),
    leaveFullscreen: () => rejected('windowControls'),
    restart: () => rejected('windowControls'),
    onResized: (_handler, onError) => unavailableSubscription('windowControls', onError),
    onFocusChanged: (_handler, onError) => unavailableSubscription('windowControls', onError),
  }),
  notifications: slot({
    permission: async () => 'unavailable' as const,
    requestPermission: async () => 'unavailable' as const,
    send: () => rejected('notifications'),
    readCustomSound: () => rejected('notifications'),
    importSound: () => rejected('notifications'),
    commitSound: () => rejected('notifications'),
    removeSound: () => rejected('notifications'),
  }),
  tray: slot({ setPendingCount: () => rejected('tray') }),
  externalLinks: slot({ open: () => rejected('externalLinks') }),
  screenCapture: slot({
    onReady: (_handler, onError) => unavailableSubscription('screenCapture', onError),
    onFinished: (_handler, onError) => unavailableSubscription('screenCapture', onError),
    onShortcut: (_handler, onError) => unavailableSubscription('screenCapture', onError),
    begin: () => rejected('screenCapture'),
    complete: () => rejected('screenCapture'),
    discard: () => rejected('screenCapture'),
  }),
  clipboardCapture: slot({
    captureOnce: () => rejected('clipboardCapture'),
    completeImage: () => rejected('clipboardCapture'),
    discardImage: () => rejected('clipboardCapture'),
  }),
  imagePaste: slot({
    subscribe: (_target, _handler, onError) =>
      unavailableSubscription('imagePaste', onError),
  }),
  serverPaths: slot({
    chooseDirectory: () => rejected('serverPaths'),
    chooseFile: () => rejected('serverPaths'),
    chooseSaveFile: () => rejected('serverPaths'),
    reveal: () => rejected('serverPaths'),
    openAttachment: () => rejected('serverPaths'),
    revealAttachment: () => rejected('serverPaths'),
    onFileDrop: (_handler, onError) => unavailableSubscription('serverPaths', onError),
    importAttachmentPath: () => rejected('serverPaths'),
  }),
  globalShortcuts: slot({
    read: () => rejected('globalShortcuts'),
    update: () => rejected('globalShortcuts'),
    reset: () => rejected('globalShortcuts'),
    setCaptureActive: () => rejected('globalShortcuts'),
    onRambleToggle: (_handler, onError) => unavailableSubscription('globalShortcuts', onError),
  }),
  speech: slot({
    start: () => ({
      id: 'unavailable-speech',
      ready: rejected('speech'),
      stop: () => rejected('speech'),
      cancel: () => rejected('speech'),
    }),
    listModels: () => rejected('speech'),
    downloadModel: () => rejected('speech'),
    deleteModel: () => rejected('speech'),
    listInputDevices: () => rejected('speech'),
    onModelProgress: (_handler, onError) => unavailableSubscription('speech', onError),
  }),
  rambleConsole: slot({
    show: () => rejected('rambleConsole'),
    restoreVisibility: () => rejected('rambleConsole'),
    hide: () => rejected('rambleConsole'),
    publish: () => rejected('rambleConsole'),
    onCommand: (_handler, onError) => unavailableSubscription('rambleConsole', onError),
    onReady: (_handler, onError) => unavailableSubscription('rambleConsole', onError),
    recordDiagnostic: () => rejected('diagnostics'),
  }),
  softwareUpdates: slot({
    version: () => rejected('softwareUpdates'),
    check: () => rejected('softwareUpdates'),
    install: () => rejected('softwareUpdates'),
    restart: () => rejected('softwareUpdates'),
  }),
  systemPermissions: slot({
    list: () => rejected('systemPermissions'),
    request: () => rejected('systemPermissions'),
    openSettings: () => rejected('systemPermissions'),
  }),
  dataStorageAdministration: slot({
    read: () => rejected('dataStorageAdministration'),
    select: () => rejected('dataStorageAdministration'),
    onProgress: (_handler, onError) =>
      unavailableSubscription('dataStorageAdministration', onError),
  }),
  hostIntegrationAdministration: slot({
    genericMcpConfiguration: () => rejected('hostIntegrationAdministration'),
    detectGenericMcpHosts: () => rejected('hostIntegrationAdministration'),
    installGenericMcpHosts: () => rejected('hostIntegrationAdministration'),
    piStatus: () => rejected('hostIntegrationAdministration'),
    installPi: () => rejected('hostIntegrationAdministration'),
    uninstallPi: () => rejected('hostIntegrationAdministration'),
    installDsh: () => rejected('hostIntegrationAdministration'),
  }),
  webAccessAdministration: slot({
    status: () => rejected('webAccessAdministration'),
    setEnabled: () => rejected('webAccessAdministration'),
    open: () => rejected('webAccessAdministration'),
    copyToken: () => rejected('webAccessAdministration'),
  }),
  diagnostics: slot({ export: () => rejected('diagnostics') }),
})

export const UNAVAILABLE_CAPABILITY_MANIFEST = UNAVAILABLE_WORKBENCH_CAPABILITIES.manifest

/** Contract-only fallback used while concrete capture capability slots migrate. */
export function createUnavailableCapturePlugins(): CapturePlugins {
  return UNAVAILABLE_CAPTURE_PLUGINS
}

export function createUnavailableWorkbenchCapabilities(): WorkbenchCapabilities {
  return UNAVAILABLE_WORKBENCH_CAPABILITIES
}

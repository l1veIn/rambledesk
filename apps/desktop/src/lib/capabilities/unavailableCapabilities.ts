import type { CapabilityName, CapabilityStatus } from './capabilityManifest'
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
    onFileDrop: (_handler, onError) => unavailableSubscription('screenCapture', onError),
    begin: () => rejected('screenCapture'),
    importServerPath: () => rejected('screenCapture'),
    complete: () => rejected('screenCapture'),
    discard: () => rejected('screenCapture'),
  }),
  clipboardCapture: slot({
    captureOnce: () => rejected('clipboardCapture'),
    completeImage: () => rejected('clipboardCapture'),
    discardImage: () => rejected('clipboardCapture'),
  }),
  serverPaths: slot({
    chooseDirectory: () => rejected('serverPaths'),
    chooseFile: () => rejected('serverPaths'),
    reveal: () => rejected('serverPaths'),
    openAttachment: () => rejected('serverPaths'),
    revealAttachment: () => rejected('serverPaths'),
  }),
  globalShortcuts: slot({
    read: () => rejected('globalShortcuts'),
    update: () => rejected('globalShortcuts'),
    reset: () => rejected('globalShortcuts'),
    setCaptureActive: () => rejected('globalShortcuts'),
    onRambleToggle: (_handler, onError) => unavailableSubscription('globalShortcuts', onError),
  }),
  speech: slot({
    start: () => rejected('speech'),
    stop: () => rejected('speech'),
    onEvent: (_handler, onError) => unavailableSubscription('speech', onError),
    listModels: () => rejected('speech'),
    downloadModel: () => rejected('speech'),
    deleteModel: () => rejected('speech'),
    listInputDevices: () => rejected('speech'),
    onModelProgress: (_handler, onError) => unavailableSubscription('speech', onError),
  }),
  rambleConsole: slot({
    show: () => rejected('rambleConsole'),
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

export function createUnavailableWorkbenchCapabilities(): WorkbenchCapabilities {
  return UNAVAILABLE_WORKBENCH_CAPABILITIES
}

import type { CapabilityStatus } from '../capabilityManifest'
import {
  createWorkbenchCapabilities,
  type CapabilitySlot,
  type WorkbenchCapabilities,
} from '../workbenchCapabilities'
import {
  createTauriDataStorageCapability,
  createTauriDiagnosticsCapability,
  createTauriHostIntegrationCapability,
  createTauriSystemPermissionCapability,
  createTauriWebAccessAdministrationCapability,
} from './administrationCapabilities'
import {
  createTauriClipboardCaptureCapability,
  createTauriScreenCaptureCapability,
} from './captureCapabilities'
import {
  createTauriExternalLinkCapability,
  createTauriServerPathCapability,
  createTauriTrayCapability,
} from './navigationCapabilities'
import { createTauriNotificationCapability } from './notificationCapability'
import { createTauriRambleConsoleCapability } from './rambleConsoleCapability'
import { createTauriShortcutCapability } from './shortcutCapability'
import { createTauriSpeechCapability } from './speechCapability'
import {
  DEFAULT_TAURI_CAPABILITY_API,
  type TauriCapabilityApi,
} from './tauriCapabilityApi'
import { createTauriUpdaterCapability } from './updaterCapability'
import { createTauriWindowCapability } from './windowCapability'

const NATIVE_AVAILABLE: CapabilityStatus = Object.freeze({
  availability: 'available',
  source: 'native',
})

function nativeSlot<Implementation>(
  implementation: Implementation,
): CapabilitySlot<Implementation> {
  return Object.freeze({ status: NATIVE_AVAILABLE, implementation })
}

/** Creates the executable Desktop capability registry; its manifest is derived from these slots. */
export function createTauriWorkbenchCapabilities(
  api: TauriCapabilityApi = DEFAULT_TAURI_CAPABILITY_API,
): WorkbenchCapabilities {
  return createWorkbenchCapabilities({
    windowControls: nativeSlot(createTauriWindowCapability(api)),
    notifications: nativeSlot(createTauriNotificationCapability(api)),
    tray: nativeSlot(createTauriTrayCapability(api)),
    externalLinks: nativeSlot(createTauriExternalLinkCapability(api)),
    screenCapture: nativeSlot(createTauriScreenCaptureCapability(api)),
    clipboardCapture: nativeSlot(createTauriClipboardCaptureCapability(api)),
    serverPaths: nativeSlot(createTauriServerPathCapability(api)),
    globalShortcuts: nativeSlot(createTauriShortcutCapability(api)),
    speech: nativeSlot(createTauriSpeechCapability(api)),
    rambleConsole: nativeSlot(createTauriRambleConsoleCapability(api)),
    softwareUpdates: nativeSlot(createTauriUpdaterCapability(api)),
    systemPermissions: nativeSlot(createTauriSystemPermissionCapability(api)),
    dataStorageAdministration: nativeSlot(createTauriDataStorageCapability(api)),
    hostIntegrationAdministration: nativeSlot(createTauriHostIntegrationCapability(api)),
    webAccessAdministration: nativeSlot(
      createTauriWebAccessAdministrationCapability(api),
    ),
    diagnostics: nativeSlot(createTauriDiagnosticsCapability(api)),
  })
}

export type { TauriCapabilityApi } from './tauriCapabilityApi'

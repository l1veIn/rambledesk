import type { FeedbackWorkspaceView } from '$lib/feedback'
import type { DiagnosticExportResult } from '$lib/nativePath'
import type { SpeechModelId } from '$lib/preferences'
import type { RambleConsoleCommand, RambleConsoleState } from '$lib/rambleConsole'
import type { ShortcutAction, ShortcutConfig } from '$lib/shortcutSettings'
import type {
  SpeechRecognitionListener,
  SpeechRecognitionSession,
} from '$lib/speech'
import type {
  ClipboardCapturePlugin,
  ImagePastePlugin,
  ScreenCapturePlugin,
} from './capturePlugin'

export type {
  AttachmentCandidate,
  AttachmentCandidateSource,
  ClipboardCapturePlugin,
  ClipboardCaptureResult,
  ImagePastePlugin,
  ScreenCaptureFinished,
  ScreenCapturePlugin,
} from './capturePlugin'

import {
  capabilityManifest,
  type CapabilityManifest,
  type CapabilityName,
  type CapabilityStatus,
} from './capabilityManifest'

export type CapabilityUnsubscribe = () => void
export type CapabilityErrorHandler = (cause: unknown) => void

export class CapabilityUnavailableError extends Error {
  constructor(
    readonly capability: CapabilityName,
    readonly status: CapabilityStatus,
  ) {
    super(`${capability} is ${status.availability} in this environment.`)
    this.name = 'CapabilityUnavailableError'
  }
}

export interface WindowCapability {
  platform(): 'macOS' | 'Windows' | 'Linux' | 'unknown'
  isMaximized(): Promise<boolean>
  minimize(): Promise<void>
  toggleMaximize(): Promise<void>
  close(): Promise<void>
  startDragging(): Promise<void>
  leaveFullscreen(): Promise<void>
  restart(): Promise<void>
  onResized(handler: () => void, onError: CapabilityErrorHandler): CapabilityUnsubscribe
  onFocusChanged(
    handler: (focused: boolean) => void,
    onError: CapabilityErrorHandler,
  ): CapabilityUnsubscribe
}

export type NotificationPermission = 'granted' | 'denied' | 'default' | 'unavailable'
export type NotificationSoundImport = Readonly<{
  id: string
  name: string
  bytes: readonly number[]
}>
export interface NotificationCapability {
  permission(): Promise<NotificationPermission>
  requestPermission(): Promise<NotificationPermission>
  send(input: Readonly<{ title: string; body: string }>): Promise<void>
  readCustomSound(id: string): Promise<ArrayBuffer>
  importSound(path: string): Promise<NotificationSoundImport>
  commitSound(id: string): Promise<void>
  removeSound(id: string): Promise<void>
}

export interface TrayCapability {
  setPendingCount(count: number): Promise<void>
}

export interface ExternalLinkCapability {
  open(url: string): Promise<void>
}

export interface ServerPathCapability {
  chooseDirectory(): Promise<string | null>
  chooseFile(input: Readonly<{ extensions: readonly string[] }>): Promise<string | null>
  chooseSaveFile(input: Readonly<{
    defaultName: string
    extensions: readonly string[]
  }>): Promise<string | null>
  reveal(path: string): Promise<void>
  openAttachment(input: Readonly<{
    requestId: string
    attachmentId: string
    kind: 'request' | 'workspace'
  }>): Promise<string>
  revealAttachment(input: Readonly<{
    requestId: string
    attachmentId: string
    kind: 'request' | 'workspace'
  }>): Promise<string>
  onFileDrop(
    handler: (event: NativeFileDropEvent) => void,
    onError: CapabilityErrorHandler,
  ): CapabilityUnsubscribe
  importAttachmentPath(input: Readonly<{
    requestId: string
    path: string
    expectedRevision: number
  }>): Promise<FeedbackWorkspaceView>
}

export type NativeFileDropEvent = Readonly<{
  type: 'enter' | 'over' | 'drop' | 'leave'
  paths: readonly string[]
}>
export interface ShortcutCapability {
  read(): Promise<ShortcutConfig>
  update(action: ShortcutAction, shortcut: string): Promise<ShortcutConfig>
  reset(): Promise<ShortcutConfig>
  setCaptureActive(active: boolean): Promise<void>
  onRambleToggle(handler: () => void, onError: CapabilityErrorHandler): CapabilityUnsubscribe
}

export type SpeechRecognitionOptions = Readonly<{
  inputDevice: string | null
  modelId: string
  vadThreshold: number
  vadSilenceMs: number
  hotwords: readonly string[]
}>
export interface SpeechRecognitionPlugin {
  start(
    options: SpeechRecognitionOptions,
    listener: SpeechRecognitionListener,
  ): SpeechRecognitionSession
}

export interface RambleConsoleCapability {
  show(): Promise<void>
  restoreVisibility(): Promise<void>
  hide(): Promise<void>
  publish(state: RambleConsoleState): Promise<void>
  onCommand(
    handler: (command: RambleConsoleCommand) => void,
    onError: CapabilityErrorHandler,
  ): CapabilityUnsubscribe
  onReady(handler: () => void, onError: CapabilityErrorHandler): CapabilityUnsubscribe
  recordDiagnostic(activity: string, caseId: string): Promise<void>
}

export type UpdateCheckInput = Readonly<{ prompt: boolean; forcePrompt: boolean }>
export interface UpdaterCapability {
  version(): Promise<string>
  check(input: UpdateCheckInput): Promise<void>
  install(): Promise<void>
  restart(): Promise<void>
}

export type MacPermissionStatus = 'granted' | 'denied' | 'not_determined' | 'unknown'
export type MacPermission = Readonly<{
  id: string
  status: MacPermissionStatus
  restart_required: boolean
}>
export interface SystemPermissionCapability {
  list(): Promise<readonly MacPermission[]>
  request(permission: string): Promise<MacPermission>
  openSettings(permission: string): Promise<void>
}

export type DataStorageView = Readonly<{
  active_path: string
  selected_path: string
  restart_required: boolean
}>
export type StorageMigrationProgress = Readonly<{ copied: number; total: number }>
export interface DataStorageCapability {
  read(): Promise<DataStorageView>
  select(path: string): Promise<DataStorageView>
  onProgress(
    handler: (progress: StorageMigrationProgress) => void,
    onError: CapabilityErrorHandler,
  ): CapabilityUnsubscribe
}

export type SpeechModelInfo = Readonly<{
  id: SpeechModelId
  engine_id: string
  display_name: string
  description: string
  size_bytes: number
  installed: boolean
  path: string
  missing_files: readonly string[]
  streaming: boolean
  hotwords_supported: boolean
  languages: readonly string[]
  license: string
}>
export type SpeechModelProgress = Readonly<{
  model_id: string
  downloaded: number
  total: number
}>
export interface SpeechAdministrationCapability {
  listModels(): Promise<readonly SpeechModelInfo[]>
  downloadModel(modelId: string): Promise<SpeechModelInfo>
  deleteModel(modelId: string): Promise<SpeechModelInfo>
  listInputDevices(): Promise<readonly string[]>
  onModelProgress(
    handler: (progress: SpeechModelProgress) => void,
    onError: CapabilityErrorHandler,
  ): CapabilityUnsubscribe
}

export type McpHostView = Readonly<{
  id: string
  name: string
  iconSvg: string
  installed: boolean
  configured: boolean
  configPath: string
  restartRequired: boolean
}>
export type McpInstallResult = Readonly<{
  hostId: string
  action: 'created' | 'updated' | 'unchanged'
  configPath: string
  restartRequired: boolean
}>
export type PiPackageStatus = Readonly<{
  cliAvailable: boolean
  installed: boolean
  sourceCount: number
  restartRequired: boolean
}>
export type DshInstallResult = Readonly<{
  profileId: string
  profileDir: string
  patchPath: string
  action: 'created' | 'updated' | 'unchanged'
  restartRequired: boolean
}>
export interface HostIntegrationCapability {
  genericMcpConfiguration(): Promise<string>
  detectGenericMcpHosts(): Promise<readonly McpHostView[]>
  installGenericMcpHosts(hostIds: readonly string[]): Promise<readonly McpInstallResult[]>
  piStatus(): Promise<PiPackageStatus>
  installPi(): Promise<string>
  uninstallPi(): Promise<string>
  installDsh(): Promise<readonly DshInstallResult[]>
}

export type WebAccessStatus = Readonly<{ running: boolean; url: string | null }>
export interface WebAccessAdministrationCapability {
  status(): Promise<WebAccessStatus>
  setEnabled(enabled: boolean): Promise<WebAccessStatus>
  open(): Promise<void>
  copyToken(): Promise<void>
}

export type DiagnosticScope = 'last_24_hours' | 'last_7_days' | 'all'
export interface DiagnosticsCapability {
  export(scope: DiagnosticScope, path: string): Promise<DiagnosticExportResult>
}

export type CapabilitySlot<Implementation> = Readonly<{
  status: CapabilityStatus
  implementation: Implementation
}>
export type WorkbenchCapabilitySlots = Readonly<{
  windowControls: CapabilitySlot<WindowCapability>
  notifications: CapabilitySlot<NotificationCapability>
  tray: CapabilitySlot<TrayCapability>
  externalLinks: CapabilitySlot<ExternalLinkCapability>
  screenCapture: CapabilitySlot<ScreenCapturePlugin>
  clipboardCapture: CapabilitySlot<ClipboardCapturePlugin>
  imagePaste: CapabilitySlot<ImagePastePlugin>
  serverPaths: CapabilitySlot<ServerPathCapability>
  globalShortcuts: CapabilitySlot<ShortcutCapability>
  speech: CapabilitySlot<SpeechRecognitionPlugin & SpeechAdministrationCapability>
  rambleConsole: CapabilitySlot<RambleConsoleCapability>
  softwareUpdates: CapabilitySlot<UpdaterCapability>
  systemPermissions: CapabilitySlot<SystemPermissionCapability>
  dataStorageAdministration: CapabilitySlot<DataStorageCapability>
  hostIntegrationAdministration: CapabilitySlot<HostIntegrationCapability>
  webAccessAdministration: CapabilitySlot<WebAccessAdministrationCapability>
  diagnostics: CapabilitySlot<DiagnosticsCapability>
}>

export type WorkbenchCapabilities = WorkbenchCapabilitySlots & Readonly<{ manifest: CapabilityManifest }>

/** Builds the only supported registry shape and derives its manifest from slot status. */
export function createWorkbenchCapabilities(slots: WorkbenchCapabilitySlots): WorkbenchCapabilities {
  return Object.freeze({ ...slots, manifest: capabilityManifest(slots) })
}

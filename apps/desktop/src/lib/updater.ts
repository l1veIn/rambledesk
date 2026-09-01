import { getVersion } from '@tauri-apps/api/app'
import { fetch as tauriFetch } from '@tauri-apps/plugin-http'
import { relaunch } from '@tauri-apps/plugin-process'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { get, writable } from 'svelte/store'

import { currentDesktopPlatform } from './platform'
import { TAURI_DESKTOP_SHELL_INSTRUMENTATION } from './desktop-shell/instrumentation'
import { isNewerReleaseVersion, normalizeUpdateNotes } from './updateVersion'

export type UpdateStatus =
  | 'idle'
  | 'checking'
  | 'up-to-date'
  | 'available'
  | 'downloading'
  | 'ready'
  | 'error'

export type UpdateState = {
  status: UpdateStatus
  version: string
  message: string
  downloaded: number
  total: number
}

export type CheckForUpdatesOptions = {
  prompt?: boolean
  forcePrompt?: boolean
}

const LATEST_MANIFEST_URL =
  'https://github.com/l1veIn/rambledesk/releases/latest/download/latest.json'

const initialState: UpdateState = {
  status: 'idle',
  version: '',
  message: '',
  downloaded: 0,
  total: 0,
}

export const updateState = writable<UpdateState>(initialState)
export const updateDialogOpen = writable(false)

let availableUpdate: Update | null = null
let activeCheck: Promise<void> | null = null
let dismissedVersion = ''

export function canInstallInAppUpdate() {
  return currentDesktopPlatform() === 'Windows'
}

export function checkForUpdates(options: CheckForUpdatesOptions = {}): Promise<void> {
  if (activeCheck) {
    return activeCheck.then(() => {
      maybePromptForUpdate(options)
    })
  }
  activeCheck = checkForUpdatesNow(options).finally(() => {
    activeCheck = null
  })
  return activeCheck
}

async function checkForUpdatesNow(options: CheckForUpdatesOptions) {
  if (previewUpdateRequested()) {
    applyAvailableUpdate('0.0.3', previewUpdateNotes())
    maybePromptForUpdate(options)
    return
  }
  if (!('__TAURI_INTERNALS__' in window) || import.meta.env.DEV) {
    updateState.set({ ...initialState, status: 'up-to-date' })
    return
  }
  updateState.set({ ...initialState, status: 'checking' })
  try {
    if (canInstallInAppUpdate()) await checkWindowsUpdate()
    else await checkManifestUpdate()
    maybePromptForUpdate(options)
  } catch (cause) {
    const message = messageFrom(cause)
    updateState.set({ ...initialState, status: 'error', message })
    void logUpdaterError(message)
  }
}

async function checkWindowsUpdate() {
  availableUpdate?.close?.()
  availableUpdate = await check()
  if (!availableUpdate) {
    updateState.set({ ...initialState, status: 'up-to-date' })
    return
  }
  applyAvailableUpdate(availableUpdate.version, availableUpdate.body ?? '')
}

async function checkManifestUpdate() {
  availableUpdate?.close?.()
  availableUpdate = null
  const current = await getVersion()
  const response = await tauriFetch(LATEST_MANIFEST_URL, { connectTimeout: 15_000 })
  if (!response.ok) {
    throw new Error(`Update manifest returned HTTP ${response.status}`)
  }
  const manifest = (await response.json()) as { version?: unknown; notes?: unknown }
  const latest = typeof manifest.version === 'string' ? manifest.version : ''
  if (!latest || !isNewerReleaseVersion(latest, current)) {
    updateState.set({ ...initialState, status: 'up-to-date' })
    return
  }
  applyAvailableUpdate(latest, typeof manifest.notes === 'string' ? manifest.notes : '')
}

function applyAvailableUpdate(version: string, notes: string) {
  updateState.set({
    ...initialState,
    status: 'available',
    version,
    message: normalizeUpdateNotes(notes),
  })
}

function maybePromptForUpdate(options: CheckForUpdatesOptions) {
  if (!options.prompt) return
  const current = get(updateState)
  if (current.status !== 'available' && current.status !== 'ready') return
  if (!options.forcePrompt && current.version && current.version === dismissedVersion) return
  updateDialogOpen.set(true)
}

export function dismissUpdateDialog() {
  const current = get(updateState)
  if (current.status === 'available' && current.version) {
    dismissedVersion = current.version
  }
  updateDialogOpen.set(false)
}

export function openUpdateDialog() {
  updateDialogOpen.set(true)
}

export async function downloadAndInstallUpdate(): Promise<void> {
  if (!availableUpdate) {
    await checkForUpdates({ prompt: true, forcePrompt: true })
    if (!availableUpdate) return
  }
  const version = availableUpdate.version
  const message = get(updateState).message || normalizeUpdateNotes(availableUpdate.body ?? '')
  let downloaded = 0
  let total = 0
  updateDialogOpen.set(true)
  updateState.set({ status: 'downloading', version, message, downloaded, total })
  try {
    await availableUpdate.downloadAndInstall((event) => {
      if (event.event === 'Started') total = event.data.contentLength ?? 0
      else if (event.event === 'Progress') downloaded += event.data.chunkLength
      updateState.set({ status: 'downloading', version, message, downloaded, total })
    })
    updateState.set({ status: 'ready', version, message, downloaded, total })
  } catch (cause) {
    const error = messageFrom(cause)
    updateState.set({ status: 'error', version, message: error, downloaded, total })
    void logUpdaterError(error)
  }
}

export async function restartAfterUpdate(): Promise<void> {
  await relaunch()
}

function previewUpdateRequested() {
  try {
    return new URLSearchParams(window.location.search).get('dialog') === 'update'
  } catch {
    return false
  }
}

function previewUpdateNotes() {
  return [
    "## What's Changed",
    '* Show release notes in an update dialog after launch.',
    '* Check for updates automatically when the app opens.',
    '* Keep the About page as a manual fallback.',
  ].join('\n')
}

function messageFrom(cause: unknown) {
  if (cause instanceof Error) return cause.message
  if (cause && typeof cause === 'object' && 'message' in cause) {
    return String((cause as { message: unknown }).message)
  }
  return String(cause)
}

async function logUpdaterError(message: string) {
  await TAURI_DESKTOP_SHELL_INSTRUMENTATION.reportFrontendError('updater', message)
}

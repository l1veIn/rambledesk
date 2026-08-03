import { invoke } from '@tauri-apps/api/core'
import { relaunch } from '@tauri-apps/plugin-process'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { writable } from 'svelte/store'

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

const initialState: UpdateState = {
  status: 'idle',
  version: '',
  message: '',
  downloaded: 0,
  total: 0,
}

export const updateState = writable<UpdateState>(initialState)

let availableUpdate: Update | null = null
let activeCheck: Promise<void> | null = null

export function checkForUpdates(): Promise<void> {
  if (activeCheck) return activeCheck
  activeCheck = checkForUpdatesNow().finally(() => {
    activeCheck = null
  })
  return activeCheck
}

async function checkForUpdatesNow() {
  if (!('__TAURI_INTERNALS__' in window) || import.meta.env.DEV) {
    updateState.set({ ...initialState, status: 'up-to-date' })
    return
  }
  updateState.set({ ...initialState, status: 'checking' })
  try {
    availableUpdate?.close?.()
    availableUpdate = await check()
    if (!availableUpdate) {
      updateState.set({ ...initialState, status: 'up-to-date' })
      return
    }
    updateState.set({
      ...initialState,
      status: 'available',
      version: availableUpdate.version,
      message: availableUpdate.body ?? '',
    })
  } catch (cause) {
    const message = messageFrom(cause)
    updateState.set({ ...initialState, status: 'error', message })
    void logUpdaterError(message)
  }
}

export async function downloadAndInstallUpdate(): Promise<void> {
  if (!availableUpdate) {
    await checkForUpdates()
    if (!availableUpdate) return
  }
  const version = availableUpdate.version
  let downloaded = 0
  let total = 0
  updateState.set({ status: 'downloading', version, message: '', downloaded, total })
  try {
    await availableUpdate.downloadAndInstall((event) => {
      if (event.event === 'Started') total = event.data.contentLength ?? 0
      else if (event.event === 'Progress') downloaded += event.data.chunkLength
      updateState.set({ status: 'downloading', version, message: '', downloaded, total })
    })
    updateState.set({ status: 'ready', version, message: '', downloaded, total })
  } catch (cause) {
    const message = messageFrom(cause)
    updateState.set({ status: 'error', version, message, downloaded, total })
    void logUpdaterError(message)
  }
}

export async function restartAfterUpdate(): Promise<void> {
  await relaunch()
}

function messageFrom(cause: unknown) {
  if (cause instanceof Error) return cause.message
  if (cause && typeof cause === 'object' && 'message' in cause) {
    return String((cause as { message: unknown }).message)
  }
  return String(cause)
}

async function logUpdaterError(message: string) {
  await invoke('log_frontend_error', { context: 'updater', message }).catch(() => undefined)
}

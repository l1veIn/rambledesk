import { getVersion } from '@tauri-apps/api/app'
import { invoke } from '@tauri-apps/api/core'
import { emitTo, listen } from '@tauri-apps/api/event'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { open, save } from '@tauri-apps/plugin-dialog'
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification'
import { openUrl } from '@tauri-apps/plugin-opener'

import {
  checkForUpdates,
  downloadAndInstallUpdate,
  restartAfterUpdate,
} from '$lib/updater'

export type TauriEvent<Event> = Readonly<{ payload: Event }>
export type TauriUnlisten = () => void

export interface TauriCapabilityApi {
  invoke<Result>(command: string, args?: Record<string, unknown>): Promise<Result>
  listen<Event>(
    event: string,
    handler: (event: TauriEvent<Event>) => void,
  ): Promise<TauriUnlisten>
  emitTo(target: string, event: string, payload?: unknown): Promise<void>
  currentWindow(): Readonly<{
    isMaximized(): Promise<boolean>
    isFullscreen(): Promise<boolean>
    setFullscreen(fullscreen: boolean): Promise<void>
    minimize(): Promise<void>
    toggleMaximize(): Promise<void>
    close(): Promise<void>
    startDragging(): Promise<void>
    onResized(handler: () => void): Promise<TauriUnlisten>
    onFocusChanged(handler: (event: TauriEvent<boolean>) => void): Promise<TauriUnlisten>
  }>
  currentWebview(): Readonly<{
    onDragDropEvent(
      handler: (event: TauriEvent<{
        type: 'enter' | 'over' | 'drop' | 'leave'
        paths: string[]
      }>) => void,
    ): Promise<TauriUnlisten>
  }>
  choosePath(options: Record<string, unknown>): Promise<string | string[] | null>
  savePath(options: Record<string, unknown>): Promise<string | null>
  notificationPermissionGranted(): Promise<boolean>
  requestNotificationPermission(): Promise<'granted' | 'denied' | 'default'>
  sendNotification(input: Readonly<{ title: string; body: string }>): void
  openUrl(url: string): Promise<void>
  getVersion(): Promise<string>
  checkForUpdates(input: Readonly<{ prompt: boolean; forcePrompt: boolean }>): Promise<void>
  installUpdate(): Promise<void>
  restartAfterUpdate(): Promise<void>
}

export const DEFAULT_TAURI_CAPABILITY_API: TauriCapabilityApi = {
  invoke,
  listen,
  emitTo,
  currentWindow: getCurrentWindow,
  currentWebview: () => {
    const webview = getCurrentWebview()
    return {
      onDragDropEvent: (handler) =>
        webview.onDragDropEvent(({ payload }) => {
          handler({
            payload: {
              type: payload.type,
              paths: 'paths' in payload ? payload.paths : [],
            },
          })
        }),
    }
  },
  choosePath: open as TauriCapabilityApi['choosePath'],
  savePath: save as TauriCapabilityApi['savePath'],
  notificationPermissionGranted: isPermissionGranted,
  requestNotificationPermission: requestPermission,
  sendNotification,
  openUrl,
  getVersion,
  checkForUpdates: (input) => checkForUpdates(input),
  installUpdate: downloadAndInstallUpdate,
  restartAfterUpdate,
}

import type {
  NotificationCapability,
  NotificationPermission,
  NotificationSoundImport,
} from '../workbenchCapabilities'
import type { TauriCapabilityApi } from './tauriCapabilityApi'

type NotificationSoundImportWire = Readonly<{
  id: string
  name: string
  bytes: number[]
}>

export function createTauriNotificationCapability(
  api: TauriCapabilityApi,
): NotificationCapability {
  return {
    async permission(): Promise<NotificationPermission> {
      return (await api.notificationPermissionGranted()) ? 'granted' : 'default'
    },
    async requestPermission(): Promise<NotificationPermission> {
      if (await api.notificationPermissionGranted()) return 'granted'
      return api.requestNotificationPermission()
    },
    async send(input) {
      api.sendNotification(input)
    },
    async readCustomSound(id) {
      const bytes = await api.invoke<number[]>('read_notification_sound', { id })
      return Uint8Array.from(bytes).buffer
    },
    async importSound(path): Promise<NotificationSoundImport> {
      return api.invoke<NotificationSoundImportWire>('import_notification_sound', { path })
    },
    commitSound: (id) => api.invoke<void>('commit_notification_sound', { id }),
    removeSound: (id) => api.invoke<void>('remove_notification_sound', { id }),
  }
}

import { writable } from 'svelte/store'

import type {
  CapabilitySlot,
  NotificationCapability,
} from '../capabilities/workbenchCapabilities'
import { notificationStateForPermission, type NotificationState } from '../notifications'

export type NotificationPermissionControllerContext = {
  notifications: CapabilitySlot<NotificationCapability>
  isMac: () => boolean
  getPopupEnabled: () => boolean
  setPopupEnabled: (enabled: boolean) => void
}

/**
 * OS notification permission as the workbench reads it: `checking` until the
 * platform answers, then enabled/muted/disabled, or unavailable when the
 * platform cannot answer at all.
 */
export function createNotificationPermissionController(
  context: NotificationPermissionControllerContext,
) {
  const store = writable<NotificationState>('checking')

  async function refresh() {
    try {
      const granted = (await context.notifications.implementation.permission()) === 'granted'
      if (context.isMac() && !granted && context.getPopupEnabled()) {
        context.setPopupEnabled(false)
      }
      store.set(notificationStateForPermission(granted, context.getPopupEnabled()))
    } catch {
      store.set('unavailable')
    }
  }

  function markUnavailable() {
    store.set('unavailable')
  }

  return { subscribe: store.subscribe, refresh, markUnavailable }
}

export type NotificationPermissionController = ReturnType<
  typeof createNotificationPermissionController
>

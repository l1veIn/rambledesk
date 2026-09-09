import { get } from 'svelte/store'
import { describe, expect, it, vi } from 'vitest'

import type { CapabilitySlot, NotificationCapability } from '../capabilities/workbenchCapabilities'
import { createNotificationPermissionController } from './notificationPermissionController'

function harness(permission: () => Promise<'granted' | 'denied'>) {
  const setPopupEnabled = vi.fn()
  const controller = createNotificationPermissionController({
    notifications: {
      status: { availability: 'available', source: 'native' },
      implementation: { permission },
    } as unknown as CapabilitySlot<NotificationCapability>,
    isMac: () => true,
    getPopupEnabled: () => true,
    setPopupEnabled,
  })
  return { controller, setPopupEnabled }
}

describe('notification permission controller', () => {
  it('reports enabled when the platform grants permission and popups are on', async () => {
    const { controller } = harness(async () => 'granted')
    await controller.refresh()
    expect(get(controller)).toBe('enabled')
  })

  it('mutes instead of disabling when popups are off', async () => {
    const controller = createNotificationPermissionController({
      notifications: {
        status: { availability: 'available', source: 'native' },
        implementation: { permission: async () => 'granted' },
      } as unknown as CapabilitySlot<NotificationCapability>,
      isMac: () => false,
      getPopupEnabled: () => false,
      setPopupEnabled: vi.fn(),
    })
    await controller.refresh()
    expect(get(controller)).toBe('muted')
  })

  it('turns the macOS popup preference off when permission is denied', async () => {
    const { controller, setPopupEnabled } = harness(async () => 'denied')
    await controller.refresh()
    expect(get(controller)).toBe('disabled')
    expect(setPopupEnabled).toHaveBeenCalledWith(false)
  })

  it('reports unavailable when the platform cannot answer', async () => {
    const { controller } = harness(async () => {
      throw new Error('no platform')
    })
    await controller.refresh()
    expect(get(controller)).toBe('unavailable')
  })

  it('can be marked unavailable without asking the platform', () => {
    const { controller } = harness(async () => 'granted')
    controller.markUnavailable()
    expect(get(controller)).toBe('unavailable')
  })
})

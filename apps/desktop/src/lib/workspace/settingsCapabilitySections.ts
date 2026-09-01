import type { CapabilityManifest } from '$lib/capabilities/capabilityManifest'
import type { WindowCapability } from '$lib/capabilities/workbenchCapabilities'
import type { SettingsSection } from '$lib/workbench/types'

export type SettingsSectionAvailability = Readonly<Record<SettingsSection, boolean>>

export function settingsSectionAvailability(
  manifest: CapabilityManifest,
  platform: ReturnType<WindowCapability['platform']>,
): SettingsSectionAvailability {
  return Object.freeze({
    general: true,
    permissions:
      platform === 'macOS' && manifest.systemPermissions.availability !== 'unavailable',
    notifications: manifest.notifications.availability !== 'unavailable',
    voice: manifest.speech.availability !== 'unavailable',
    'post-processing': true,
    shortcuts: manifest.globalShortcuts.availability !== 'unavailable',
    adapters: manifest.hostIntegrationAdministration.availability !== 'unavailable',
    about: true,
  })
}

export type SettingsSectionResolution = Readonly<{
  activeSection: SettingsSection
  showDesktopOnlyNotice: boolean
}>

export function resolveSettingsSection(
  requestedSection: SettingsSection,
  availability: SettingsSectionAvailability,
): SettingsSectionResolution {
  if (availability[requestedSection]) {
    return { activeSection: requestedSection, showDesktopOnlyNotice: false }
  }
  return { activeSection: 'general', showDesktopOnlyNotice: true }
}

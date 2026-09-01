import { describe, expect, it } from 'vitest'

import type { CapabilityManifest, CapabilityName } from '$lib/capabilities/capabilityManifest'
import { createUnavailableWorkbenchCapabilities } from '$lib/capabilities/unavailableCapabilities'

import {
  resolveSettingsSection,
  settingsSectionAvailability,
} from './settingsCapabilitySections'

const unavailableManifest = createUnavailableWorkbenchCapabilities().manifest

function manifestWithNativeCapabilities(...names: CapabilityName[]): CapabilityManifest {
  const manifest = { ...unavailableManifest }
  for (const name of names) {
    manifest[name] = { availability: 'available', source: 'native' }
  }
  return manifest
}

describe('settings capability sections', () => {
  it('keeps only shared settings sections in an unavailable browser registry', () => {
    const availability = settingsSectionAvailability(unavailableManifest, 'unknown')

    expect(
      Object.entries(availability)
        .filter(([, available]) => available)
        .map(([section]) => section),
    ).toEqual(['general', 'post-processing', 'about'])
  })

  it('projects each native section from its own capability', () => {
    const availability = settingsSectionAvailability(
      manifestWithNativeCapabilities(
        'systemPermissions',
        'notifications',
        'speech',
        'globalShortcuts',
        'hostIntegrationAdministration',
      ),
      'macOS',
    )

    expect(availability).toMatchObject({
      permissions: true,
      notifications: true,
      voice: true,
      shortcuts: true,
      adapters: true,
    })
  })

  it('does not expose macOS permissions on another platform', () => {
    const availability = settingsSectionAvailability(
      manifestWithNativeCapabilities('systemPermissions'),
      'Windows',
    )

    expect(availability.permissions).toBe(false)
  })

  it('falls back unsupported restored sections to General with a notice intent', () => {
    const availability = settingsSectionAvailability(unavailableManifest, 'unknown')

    expect(resolveSettingsSection('voice', availability)).toEqual({
      activeSection: 'general',
      showDesktopOnlyNotice: true,
    })
    expect(resolveSettingsSection('about', availability)).toEqual({
      activeSection: 'about',
      showDesktopOnlyNotice: false,
    })
  })
})

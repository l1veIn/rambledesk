export type CapabilityUnavailableReason =
  | 'unsupported_environment'
  | 'permission_denied'
  | 'needs_restart'
  | 'no_device'
  | 'temporarily_unavailable'

export type CapabilitySource = 'native' | 'browser' | 'none'

export type CapabilityStatus =
  | Readonly<{
      availability: 'available'
      source: Exclude<CapabilitySource, 'none'>
    }>
  | Readonly<{
      availability: 'degraded'
      source: Exclude<CapabilitySource, 'none'>
      reason: CapabilityUnavailableReason
    }>
  | Readonly<{
      availability: 'unavailable'
      source: 'none'
      reason: CapabilityUnavailableReason
    }>

export const CAPABILITY_NAMES = [
  'windowControls',
  'notifications',
  'tray',
  'externalLinks',
  'screenCapture',
  'clipboardCapture',
  'imagePaste',
  'serverPaths',
  'globalShortcuts',
  'speech',
  'rambleConsole',
  'softwareUpdates',
  'systemPermissions',
  'dataStorageAdministration',
  'hostIntegrationAdministration',
  'webAccessAdministration',
  'diagnostics',
] as const

export type CapabilityName = (typeof CAPABILITY_NAMES)[number]

export type CapabilityManifest = Readonly<Record<CapabilityName, CapabilityStatus>>
export type CapabilityRegistryProjection = Readonly<
  Record<CapabilityName, Readonly<{ status: CapabilityStatus }>>
>

/** Manifest values are always projected from executable registry slots. */
export function capabilityManifest(registry: CapabilityRegistryProjection): CapabilityManifest {
  return Object.freeze(
    Object.fromEntries(
      CAPABILITY_NAMES.map((name) => [name, Object.freeze({ ...registry[name].status })]),
    ) as Record<CapabilityName, CapabilityStatus>,
  )
}

export function capabilityAvailable(manifest: CapabilityManifest, name: CapabilityName): boolean {
  return manifest[name].availability !== 'unavailable'
}

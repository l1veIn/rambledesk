import type {
  CapabilitySlot,
  ExternalLinkCapability,
} from '../capabilities/workbenchCapabilities'

export const PROJECT_URL = 'https://github.com/l1veIn/rambledesk'
export const RELEASES_URL = `${PROJECT_URL}/releases`

/** Opens the release page in the platform browser. */
export function openReleases(
  externalLinks: CapabilitySlot<ExternalLinkCapability>,
): Promise<void> {
  return externalLinks.implementation.open(RELEASES_URL)
}

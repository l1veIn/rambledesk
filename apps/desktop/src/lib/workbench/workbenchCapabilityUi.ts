import type { CapabilityStatus } from '$lib/capabilities/capabilityManifest'

type NativeCaptureCapabilityStatuses = Readonly<{
  screenCapture: CapabilityStatus
  clipboardCapture: CapabilityStatus
}>

function usable(status: CapabilityStatus): boolean {
  return status.availability !== 'unavailable'
}

export function voiceRambleAvailable(speech: CapabilityStatus): boolean {
  return usable(speech)
}

export function nativeCaptureAvailable(statuses: NativeCaptureCapabilityStatuses): boolean {
  return usable(statuses.screenCapture) && usable(statuses.clipboardCapture)
}

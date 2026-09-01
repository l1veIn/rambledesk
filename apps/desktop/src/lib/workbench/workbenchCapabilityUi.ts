import type { CapabilityStatus } from '$lib/capabilities/capabilityManifest'

type VoiceRambleCapabilityStatuses = Readonly<{
  speech: CapabilityStatus
  rambleConsole: CapabilityStatus
}>

type NativeCaptureCapabilityStatuses = Readonly<{
  screenCapture: CapabilityStatus
  clipboardCapture: CapabilityStatus
}>

function usable(status: CapabilityStatus): boolean {
  return status.availability !== 'unavailable'
}

export function voiceRambleAvailable(statuses: VoiceRambleCapabilityStatuses): boolean {
  return usable(statuses.speech) && usable(statuses.rambleConsole)
}

export function nativeCaptureAvailable(statuses: NativeCaptureCapabilityStatuses): boolean {
  return usable(statuses.screenCapture) && usable(statuses.clipboardCapture)
}

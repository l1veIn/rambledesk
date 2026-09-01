import { describe, expect, it } from 'vitest'

import type { CapabilityStatus } from '$lib/capabilities/capabilityManifest'
import { nativeCaptureAvailable, voiceRambleAvailable } from './workbenchCapabilityUi'

const available = { availability: 'available', source: 'native' } satisfies CapabilityStatus
const degraded = {
  availability: 'degraded',
  source: 'native',
  reason: 'permission_denied',
} satisfies CapabilityStatus
const unavailable = {
  availability: 'unavailable',
  source: 'none',
  reason: 'unsupported_environment',
} satisfies CapabilityStatus

describe('workbench capability presentation', () => {
  it('shows voice Ramble whenever the platform speech plugin is usable', () => {
    expect(voiceRambleAvailable(available)).toBe(true)
    expect(voiceRambleAvailable(degraded)).toBe(true)
    expect(voiceRambleAvailable(unavailable)).toBe(false)
  })

  it('shows native capture actions only when screen and clipboard capture are usable', () => {
    expect(nativeCaptureAvailable({ screenCapture: available, clipboardCapture: available })).toBe(true)
    expect(nativeCaptureAvailable({ screenCapture: degraded, clipboardCapture: available })).toBe(true)
    expect(nativeCaptureAvailable({ screenCapture: unavailable, clipboardCapture: available })).toBe(false)
    expect(nativeCaptureAvailable({ screenCapture: available, clipboardCapture: unavailable })).toBe(false)
  })
})

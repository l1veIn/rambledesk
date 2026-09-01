import { describe, expect, it, vi } from 'vitest'

import {
  CAPABILITY_NAMES,
  capabilityAvailable,
} from './capabilityManifest'
import {
  UNAVAILABLE_CAPABILITY_MANIFEST,
  createUnavailableWorkbenchCapabilities,
} from './unavailableCapabilities'
import { CapabilityUnavailableError } from './workbenchCapabilities'

describe('capability contracts', () => {
  it('keeps the unavailable manifest complete and immutable', () => {
    expect(Object.keys(UNAVAILABLE_CAPABILITY_MANIFEST).sort()).toEqual([...CAPABILITY_NAMES].sort())
    expect(Object.isFrozen(UNAVAILABLE_CAPABILITY_MANIFEST)).toBe(true)
    expect(CAPABILITY_NAMES.every((name) => Object.isFrozen(UNAVAILABLE_CAPABILITY_MANIFEST[name]))).toBe(true)
    expect(UNAVAILABLE_CAPABILITY_MANIFEST).toMatchSnapshot()
    expect(CAPABILITY_NAMES.every((name) => !capabilityAvailable(UNAVAILABLE_CAPABILITY_MANIFEST, name))).toBe(true)
  })

  it('projects the manifest from executable registry slots', () => {
    const capabilities = createUnavailableWorkbenchCapabilities()
    expect(capabilities.manifest.screenCapture).toEqual(capabilities.screenCapture.status)
    expect(capabilities.manifest.externalLinks).toEqual(capabilities.externalLinks.status)
  })

  it('fails unavailable operations with a typed capability error', async () => {
    const capabilities = createUnavailableWorkbenchCapabilities()
    await expect(capabilities.windowControls.implementation.close()).rejects.toMatchObject({
      name: 'CapabilityUnavailableError',
      capability: 'windowControls',
      status: { availability: 'unavailable', reason: 'unsupported_environment' },
    })
    await expect(capabilities.screenCapture.implementation.begin()).rejects.toMatchObject({
      name: 'CapabilityUnavailableError',
      capability: 'screenCapture',
    })
  })

  it('reports subscription failure asynchronously and respects early unsubscribe', async () => {
    const capabilities = createUnavailableWorkbenchCapabilities()
    const onError = vi.fn()
    const unsubscribe = capabilities.speech.implementation.onEvent(vi.fn(), onError)
    unsubscribe()
    await Promise.resolve()
    expect(onError).not.toHaveBeenCalled()

    capabilities.screenCapture.implementation.onReady(vi.fn(), onError)
    await Promise.resolve()
    expect(onError).toHaveBeenCalledWith(expect.any(CapabilityUnavailableError))
  })
})

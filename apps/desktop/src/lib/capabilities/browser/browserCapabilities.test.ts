import { describe, expect, it, vi } from 'vitest'

import { CAPABILITY_NAMES } from '../capabilityManifest'
import { CapabilityUnavailableError } from '../workbenchCapabilities'
import { createBrowserWorkbenchCapabilities } from './browserCapabilities'

describe('browser Workbench capabilities', () => {
  it('advertises the implemented external-link and image-paste capabilities', async () => {
    const open = vi.fn(() => null)
    const capabilities = createBrowserWorkbenchCapabilities({
      pageUrl: 'https://workbench.example/app',
      open,
    })

    expect(capabilities.manifest.externalLinks).toEqual({
      availability: 'available',
      source: 'browser',
    })
    expect(capabilities.manifest.imagePaste).toEqual({
      availability: 'available',
      source: 'browser',
    })
    for (const name of CAPABILITY_NAMES.filter(
      (name) => name !== 'externalLinks' && name !== 'imagePaste',
    )) {
      expect(capabilities.manifest[name]).toEqual({
        availability: 'unavailable',
        source: 'none',
        reason: 'unsupported_environment',
      })
    }
    await expect(capabilities.serverPaths.implementation.chooseDirectory()).rejects.toBeInstanceOf(
      CapabilityUnavailableError,
    )
    await expect(
      capabilities.clipboardCapture.implementation.captureOnce(),
    ).rejects.toBeInstanceOf(CapabilityUnavailableError)
  })

  it('opens safe links in an isolated browsing context', async () => {
    const opened = { opener: {} } as WindowProxy
    const open = vi.fn(() => opened)
    const capabilities = createBrowserWorkbenchCapabilities({
      pageUrl: 'https://workbench.example/app',
      open,
    })

    await capabilities.externalLinks.implementation.open('https://github.com/l1veIn/rambledesk')
    expect(open).toHaveBeenCalledWith(
      'https://github.com/l1veIn/rambledesk',
      '_blank',
      'noopener,noreferrer',
    )
    expect(opened.opener).toBeNull()
  })

  it.each(['javascript:alert(1)', 'data:text/html,unsafe', 'file:///tmp/secret'])(
    'rejects the unsafe %s protocol before opening a window',
    async (url) => {
      const open = vi.fn(() => null)
      const capabilities = createBrowserWorkbenchCapabilities({
        pageUrl: 'https://workbench.example/app',
        open,
      })

      await expect(capabilities.externalLinks.implementation.open(url)).rejects.toThrow(
        'protocol is not allowed',
      )
      expect(open).not.toHaveBeenCalled()
    },
  )

  it('resolves ordinary same-origin paths without exposing opener access', async () => {
    const open = vi.fn(() => null)
    const capabilities = createBrowserWorkbenchCapabilities({
      pageUrl: 'https://workbench.example/app/session',
      open,
    })

    await capabilities.externalLinks.implementation.open('/documentation')
    expect(open).toHaveBeenCalledWith(
      'https://workbench.example/documentation',
      '_blank',
      'noopener,noreferrer',
    )
  })
})

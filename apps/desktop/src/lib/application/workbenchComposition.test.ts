import { describe, expect, it, vi } from 'vitest'

import { HttpApplicationSession, HttpApplicationTransport } from './httpApplicationTransport'
import { TestApplicationTransport } from './testApplicationTransport'
import { UnavailableApplicationTransport } from './unavailableApplicationTransport'
import { createWorkbenchComposition } from './workbenchComposition'
import { UNAVAILABLE_CAPABILITY_MANIFEST } from '../capabilities/unavailableCapabilities'
import { createBrowserWorkbenchCapabilities } from '../capabilities/browser/browserCapabilities'

describe('createWorkbenchComposition', () => {
  it('selects the Tauri implementation supplied by the desktop composition root', () => {
    const desktopTransport = new TestApplicationTransport(undefined, { initiallyReady: true })
    const composition = createWorkbenchComposition({
      environment: 'desktop',
      previewMode: false,
      desktopTransport,
    })

    expect(composition.applicationTransport).toBe(desktopTransport)
    expect(composition.capabilities.manifest).toBe(UNAVAILABLE_CAPABILITY_MANIFEST)
  })

  it('selects HTTP only for an explicit authenticated browser session', () => {
    const session = HttpApplicationSession.authenticated({
      accessToken: 'session-token',
      pageUrl: 'https://workbench.example/app',
      fetch: vi.fn<typeof fetch>(),
    })
    const composition = createWorkbenchComposition({
      environment: 'browser',
      previewMode: false,
      authenticatedWebSession: session,
    })

    expect(composition.applicationTransport).toBeInstanceOf(HttpApplicationTransport)
    expect(composition.applicationTransport.capabilities()).toBe(composition.capabilities.manifest)
  })

  it('reports the same browser manifest through the registry and HTTP transport', () => {
    const capabilities = createBrowserWorkbenchCapabilities({
      pageUrl: 'https://workbench.example/app',
      open: vi.fn(() => null),
    })
    const session = HttpApplicationSession.authenticated({
      accessToken: 'session-token',
      pageUrl: 'https://workbench.example/app',
      fetch: vi.fn<typeof fetch>(),
    })
    const composition = createWorkbenchComposition({
      environment: 'browser',
      previewMode: false,
      authenticatedWebSession: session,
      capabilities,
    })

    expect(composition.capabilities).toBe(capabilities)
    expect(composition.applicationTransport.capabilities()).toBe(capabilities.manifest)
    expect(composition.applicationTransport.capabilities().externalLinks).toEqual({
      availability: 'available',
      source: 'browser',
    })
  })

  it('keeps preview fixtures offline even if a web session is present', async () => {
    const fetchImplementation = vi.fn<typeof fetch>()
    const session = HttpApplicationSession.authenticated({
      accessToken: 'session-token',
      pageUrl: 'https://workbench.example/app',
      fetch: fetchImplementation,
    })
    const composition = createWorkbenchComposition({
      environment: 'browser',
      previewMode: true,
      authenticatedWebSession: session,
    })

    expect(composition.applicationTransport).toBeInstanceOf(UnavailableApplicationTransport)
    await expect(composition.applicationTransport.waitUntilReady()).rejects.toThrow('unavailable')
    expect(fetchImplementation).not.toHaveBeenCalled()
  })

  it('uses the unavailable implementation for an ordinary browser without a session', () => {
    const composition = createWorkbenchComposition({
      environment: 'browser',
      previewMode: false,
    })

    expect(composition.applicationTransport).toBeInstanceOf(UnavailableApplicationTransport)
  })
})

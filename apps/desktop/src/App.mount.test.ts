// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(async () => undefined) }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }))

import App from './App.svelte'
import { UnavailableApplicationTransport } from './lib/application/unavailableApplicationTransport'
import { createUnavailableWorkbenchCapabilities } from './lib/capabilities/unavailableCapabilities'

/**
 * Smoke test for the composition root. It catches the class of bug where a
 * `bind:` target is not a real store: Svelte writes bound props through
 * `store.set`, so every bound session must expose it.
 */
describe('App composition root', () => {
  beforeEach(() => {
    // Skip the onboarding wizard so the workbench shell renders.
    localStorage.setItem('rambledesk.onboarding-completed', 'true')
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as never
    // svelte-sonner reads the colour-scheme media query during mount.
    window.matchMedia = ((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
    })) as never
  })

  it('mounts the workbench shell without throwing', async () => {
    const host = document.createElement('div')
    document.body.append(host)
    const app = mount(App, {
      target: host,
      props: {
        applicationTransport: new UnavailableApplicationTransport(),
        capabilities: createUnavailableWorkbenchCapabilities(),
        publishedFeedbackAction: {
          open: async () => undefined,
          available: () => false,
        } as never,
      },
    })
    await new Promise((resolve) => setTimeout(resolve, 50))

    expect(host.querySelector('main')).not.toBeNull()
    await unmount(app)
  })
})

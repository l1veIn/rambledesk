// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('$lib/preferences', async () => {
  const { writable } = await import('svelte/store')
  return { locale: writable('en') }
})

import WorkspaceTabStripHarness from './workspaceTabStripHarness.svelte'
import { inboxViewDescriptor, sessionViewDescriptor } from './viewDescriptors'

const SCROLL_WIDTH = 1_000

async function settle() {
  await new Promise((resolve) => setTimeout(resolve, 20))
}

describe('workspace tab strip scrolling', () => {
  let host: HTMLElement
  let scrollLeft: number

  beforeEach(() => {
    scrollLeft = 0
    // jsdom has no layout and no Web Animations API.
    Element.prototype.getAnimations = (() => []) as never
    Object.defineProperty(HTMLElement.prototype, 'scrollLeft', {
      configurable: true,
      get: () => scrollLeft,
      set: (value: number) => {
        scrollLeft = value
      },
    })
    Object.defineProperty(HTMLElement.prototype, 'scrollWidth', {
      configurable: true,
      get: () => SCROLL_WIDTH,
    })
    Object.defineProperty(HTMLElement.prototype, 'clientWidth', {
      configurable: true,
      get: () => 300,
    })
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as never
    host = document.createElement('div')
    document.body.append(host)
  })

  afterEach(() => {
    host.remove()
    vi.unstubAllGlobals()
  })

  function harness(props: Record<string, unknown> = {}) {
    return mount(WorkspaceTabStripHarness, {
      target: host,
      props: { activeViewKey: 'inbox:singleton', ...props },
    }) as unknown as {
      setViews: (views: unknown[]) => void
      setActive: (viewKey: string) => void
    }
  }

  it('scrolls to the end when a tab arrives in the queue', async () => {
    const app = harness()
    await settle()
    scrollLeft = 0

    app.setViews([
      inboxViewDescriptor(),
      sessionViewDescriptor('codex', 'alpha'),
      sessionViewDescriptor('pi', 'beta'),
    ])
    await settle()

    expect(scrollLeft).toBe(SCROLL_WIDTH)
    await unmount(app as never)
  })

  it('reveals the newly active tab that is scrolled out of view', async () => {
    const app = harness({
      views: [
        inboxViewDescriptor(),
        sessionViewDescriptor('codex', 'alpha'),
        sessionViewDescriptor('pi', 'beta'),
      ],
      activeViewKey: 'session:pi:beta',
    })
    await settle()

    // The human scrolled to the end, then activated the first tab.
    scrollLeft = SCROLL_WIDTH
    app.setActive('inbox:singleton')
    await settle()

    expect(scrollLeft).toBe(0)
    await unmount(app as never)
  })

  it('does not scroll when an existing tab is closed', async () => {
    const app = harness()
    app.setViews([inboxViewDescriptor(), sessionViewDescriptor('codex', 'alpha')])
    await settle()
    scrollLeft = 0

    app.setViews([inboxViewDescriptor()])
    await settle()

    expect(scrollLeft).toBe(0)
    await unmount(app as never)
  })
})

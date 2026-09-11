// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import WorkspaceTabStripHarness from './workspaceTabStripHarness.svelte'
import { inboxViewDescriptor, sessionViewDescriptor, workspaceViewKey } from './viewDescriptors'

vi.mock('$lib/preferences', async () => {
  const { writable } = await import('svelte/store')
  return { locale: writable('en') }
})

const views = [inboxViewDescriptor(), sessionViewDescriptor('codex', 'alpha'), sessionViewDescriptor('pi', 'beta')]

async function settle() {
  await new Promise((resolve) => setTimeout(resolve, 20))
}

function key(element: HTMLElement, value: string) {
  element.dispatchEvent(new KeyboardEvent('keydown', { key: value, bubbles: true, cancelable: true }))
}

describe('workspace tab strip keyboard interaction', () => {
  let host: HTMLDivElement
  let app: ReturnType<typeof mount>

  beforeEach(() => {
    vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
    Element.prototype.getAnimations = (() => []) as never
    host = document.createElement('div')
    document.body.append(host)
  })

  afterEach(async () => {
    await unmount(app)
    host.remove()
    vi.unstubAllGlobals()
  })

  function tab(index: number) {
    return host.querySelectorAll<HTMLElement>('[role="tab"]')[index]
  }

  it('moves focus without activating, then activates and closes through the focused tab', async () => {
    const onActivate = vi.fn()
    const onClose = vi.fn()
    app = mount(WorkspaceTabStripHarness, { target: host, props: { views, onActivate, onClose } })
    await settle()
    tab(0).focus()

    key(tab(0), 'End')
    await settle()
    expect(document.activeElement).toBe(tab(2))
    expect(tab(0).getAttribute('aria-selected')).toBe('true')
    expect(onActivate).not.toHaveBeenCalled()

    key(tab(2), 'ArrowRight')
    await settle()
    expect(document.activeElement).toBe(tab(0))
    key(tab(0), 'ArrowRight')
    await settle()
    key(tab(1), 'Enter')
    await settle()
    expect(onActivate).toHaveBeenCalledExactlyOnceWith(workspaceViewKey(views[1]))
    expect(tab(1).getAttribute('aria-selected')).toBe('true')

    key(tab(1), 'Delete')
    await settle()
    expect(onClose).toHaveBeenCalledExactlyOnceWith(workspaceViewKey(views[1]))
    expect(host.querySelectorAll('[role="tab"]')).toHaveLength(2)
    expect(document.activeElement).toBe(tab(0))
    expect(tab(0).getAttribute('aria-selected')).toBe('true')
  })

  it.each([
    { disabled: true, pendingViewKey: null },
    { disabled: false, pendingViewKey: workspaceViewKey(views[1]) },
  ])('blocks activation and closing consistently while unavailable: %j', async (lock) => {
    const onActivate = vi.fn()
    const onClose = vi.fn()
    app = mount(WorkspaceTabStripHarness, { target: host, props: { views, onActivate, onClose, ...lock } })
    await settle()
    tab(0).focus()
    key(tab(0), 'ArrowRight')
    await settle()
    expect(document.activeElement).toBe(tab(1))
    key(tab(1), 'Enter')
    key(tab(1), 'Delete')
    tab(1).dispatchEvent(new MouseEvent('mousedown', { button: 1, bubbles: true, cancelable: true }))
    await settle()

    expect(onActivate).not.toHaveBeenCalled()
    expect(onClose).not.toHaveBeenCalled()
    expect(host.querySelectorAll('[role="tab"]')).toHaveLength(3)
  })

  it('closes a tab on middle-mousedown and cancels the browser autoscroll', async () => {
    const onClose = vi.fn()
    app = mount(WorkspaceTabStripHarness, { target: host, props: { views, onClose } })
    await settle()
    const target = tab(1)
    const down = new MouseEvent('mousedown', { button: 1, bubbles: true, cancelable: true })
    target.dispatchEvent(down)
    await settle()

    expect(down.defaultPrevented).toBe(true)
    expect(onClose).toHaveBeenCalledExactlyOnceWith(workspaceViewKey(views[1]))
    expect(host.querySelectorAll('[role="tab"]')).toHaveLength(2)
  })
})

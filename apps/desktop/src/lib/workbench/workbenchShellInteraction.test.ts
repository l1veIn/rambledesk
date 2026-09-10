// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import WorkbenchShellInteractionHarness from '../../dev/WorkbenchShellInteractionHarness.svelte'

vi.mock('$lib/preferences', async () => {
  const { writable } = await import('svelte/store')
  return { locale: writable('en') }
})

async function settle() {
  await new Promise((resolve) => setTimeout(resolve, 30))
}

function button(label: string) {
  const result = [...document.querySelectorAll<HTMLButtonElement>('button')]
    .find((element) => element.getAttribute('aria-label') === label || element.textContent?.trim() === label)
  if (!result) throw new Error(`Missing button: ${label}`)
  return result
}

function escape() {
  const event = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })
  document.activeElement?.dispatchEvent(event)
  return event
}

function sidebarOpener() {
  return document.querySelector<HTMLButtonElement>('[aria-controls="host-navigation-pane"]')!
}

describe('phone workbench drawer keyboard ownership', () => {
  let host: HTMLDivElement
  let app: ReturnType<typeof mount>

  beforeEach(async () => {
    vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
    vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }))
    Element.prototype.getAnimations = (() => []) as never
    host = document.createElement('div')
    document.body.append(host)
    app = mount(WorkbenchShellInteractionHarness, { target: host })
    await settle()
  })

  afterEach(async () => {
    await unmount(app)
    host.remove()
    vi.unstubAllGlobals()
  })

  it('returns Escape focus to the persistent host opener and restores workspace access', async () => {
    const opener = sidebarOpener()
    opener.focus()
    opener.click()
    await settle()
    expect(document.activeElement?.id).toBe('host-navigation-pane')
    expect(document.querySelector<HTMLElement>('#workspace-pane')?.inert).toBe(true)

    escape()
    await settle()

    expect(document.querySelector('.shell-drawer-open')).toBeNull()
    expect(document.querySelector<HTMLElement>('#workspace-pane')?.inert).toBe(false)
    expect(document.activeElement).toBe(opener)
  })

  it('returns focus to the recreated request opener after closing from inside the drawer', async () => {
    const opener = button('Open request list')
    opener.focus()
    opener.click()
    await settle()
    expect(opener.isConnected).toBe(false)
    const close = button('Collapse request list')
    close.focus()
    close.click()
    await settle()

    expect(document.activeElement).toBe(button('Open request list'))
  })

  it('returns to the request opener when a pointer opened it without focusing the button', async () => {
    expect(document.activeElement).toBe(document.body)
    button('Open request list').click()
    await settle()
    const backdrop = button('Close navigation')
    backdrop.focus()
    backdrop.click()
    await settle()

    expect(document.activeElement).toBe(button('Open request list'))
  })

  it('finds the actual titlebar opener when pointer input did not move focus to it', async () => {
    sidebarOpener().click()
    await settle()
    escape()
    await settle()

    expect(document.activeElement).toBe(sidebarOpener())
  })

  it('lets the actual nested search dialog consume Escape before the navigation drawer', async () => {
    sidebarOpener().click()
    await settle()
    const search = button('Search sessions and projects')
    search.focus()
    search.click()
    await settle()
    expect(document.querySelector('[role="dialog"]')).not.toBeNull()

    expect(escape().defaultPrevented).toBe(true)
    await settle()
    expect(document.querySelector('[role="dialog"]')).toBeNull()
    expect(document.querySelector('#host-navigation-pane')?.classList.contains('shell-drawer-open')).toBe(true)
    expect(document.activeElement).toBe(search)

    escape()
    await settle()
    expect(document.querySelector('.shell-drawer-open')).toBeNull()
  })

  it('preserves an intentional focus move to another available titlebar action', async () => {
    sidebarOpener().focus()
    sidebarOpener().click()
    await settle()
    const other = button('Other titlebar action')
    other.focus()
    other.click()
    await settle()

    expect(document.activeElement).toBe(other)
    expect(document.querySelector('.shell-drawer-open')).toBeNull()
  })

  it('excludes stale session rows during refresh while keeping navigation controls available', async () => {
    sidebarOpener().click()
    await settle()
    const session = button('Existing session · codex')
    function isInert(element: HTMLElement) {
      for (let ancestor: HTMLElement | null = element; ancestor; ancestor = ancestor.parentElement) {
        if (ancestor.inert) return true
      }
      return false
    }
    expect(isInert(session)).toBe(false)

    button('Toggle session refresh').click()
    await settle()
    expect(isInert(session)).toBe(true)
    expect(isInert(button('Search sessions and projects'))).toBe(false)
    expect(isInert(button('Collapse sidebar'))).toBe(false)

    button('Toggle session refresh').click()
    await settle()
    expect(isInert(session)).toBe(false)
  })
})

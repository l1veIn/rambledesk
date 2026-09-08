import { describe, expect, it, vi } from 'vitest'
import { createRawSnippet } from 'svelte'
import { render } from 'svelte/server'
import WorkbenchShell from './WorkbenchShell.svelte'

vi.mock('$lib/preferences', async () => {
  const { writable } = await import('svelte/store')
  return { locale: writable('en') }
})

vi.mock('$lib/uiPreferences', () => ({
  initialHostRailCollapsed: () => false,
  initialRequestRailCollapsed: () => false,
  initialHostRailWidth: () => 240,
  initialRequestRailWidth: () => 240,
  saveHostRailWidth: vi.fn(),
  saveRequestRailWidth: vi.fn(),
}))

function snippet(name: string) {
  return createRawSnippet(() => ({ render: () => `<span data-slot="${name}">${name}</span>` }))
}

function shell(extra: Record<string, unknown> = {}) {
  return render(WorkbenchShell, {
    props: {
      hostRail: snippet('host-rail'),
      requestPane: snippet('request-pane'),
      workspacePane: snippet('workspace-pane'),
      startupRecovery: snippet('startup-recovery'),
      ...extra,
    },
  }).body
}

describe('workbench shell layout', () => {
  it('renders both rails and the workspace pane', () => {
    const html = shell()
    expect(html).toContain('id="host-navigation-pane"')
    expect(html).toContain('id="request-list-pane"')
    expect(html).toContain('id="workspace-pane"')
    expect(html).toContain('data-slot="host-rail"')
    expect(html).toContain('data-slot="request-pane"')
    expect(html).toContain('data-slot="workspace-pane"')
    expect(html).not.toContain('data-slot="startup-recovery"')
  })

  it('drops the request pane when the surface is standalone', () => {
    const html = shell({ requestPaneVisible: false })
    expect(html).not.toContain('id="request-list-pane"')
    expect(html).toContain('id="host-navigation-pane"')
    expect(html).toContain('id="workspace-pane"')
  })

  it('replaces the panes with startup recovery after a failed start', () => {
    const html = shell({ startupFailed: true })
    expect(html).toContain('data-slot="startup-recovery"')
    expect(html).toContain('id="host-navigation-pane"')
    expect(html).not.toContain('id="request-list-pane"')
    expect(html).not.toContain('id="workspace-pane"')
  })

  it('fits the stored rail widths into the navigation budget', () => {
    const html = shell()
    expect(html).toContain('style="width: 240px"')
  })

  it('keeps the columns and hides the drawer affordances on desktop', () => {
    const html = shell({ mode: 'desktop' })
    expect(html).not.toContain('shell-drawer')
    expect(html).not.toContain('shell-fab')
    expect(html).not.toContain('shell-drawer-backdrop')
  })

  it('turns the rails into drawers on phones and offers reopen affordances', () => {
    const closed = shell({ mode: 'phone', hostCollapsed: true, requestCollapsed: true })
    expect(closed).toContain('shell-drawer')
    expect(closed).not.toContain('shell-drawer-open')
    expect(closed).not.toContain('width: 240px')
    expect(closed).toContain('aria-label="Open request list"')
    expect(closed).toContain('shell-fab-left')
    expect(closed).not.toContain('shell-drawer-backdrop')

    const open = shell({ mode: 'phone', hostCollapsed: false, requestCollapsed: true })
    expect(open).toContain('shell-drawer-open')
    expect(open).toContain('shell-drawer-backdrop')
    expect(open).not.toContain('aria-label="Open request list"')
  })

  it('lifts the phone request drawer out of the workspace stacking context', () => {
    const html = shell({ mode: 'phone', requestCollapsed: true })
    // `.appearance-workspace` isolates its own stacking context, so the drawer must render
    // after it — otherwise the backdrop would paint over the drawer.
    expect(html.indexOf('id="request-list-pane"')).toBeGreaterThan(html.indexOf('id="workspace-pane"'))
  })

  it('offers no request drawer when the request pane is unavailable', () => {
    const html = shell({ mode: 'phone', requestPaneVisible: false, requestCollapsed: true })
    expect(html).not.toContain('id="request-list-pane"')
    expect(html).not.toContain('aria-label="Open request list"')
  })
})

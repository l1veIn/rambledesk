import { describe, expect, it, vi } from 'vitest'
import { render } from 'svelte/server'

vi.mock('$lib/preferences', async () => {
  const { writable } = await import('svelte/store')
  return { locale: writable('en') }
})

import WorkspaceTabStrip from './WorkspaceTabStrip.svelte'
import { inboxViewDescriptor, sessionViewDescriptor } from './viewDescriptors'

function strip() {
  return render(WorkspaceTabStrip, {
    props: {
      views: [
        inboxViewDescriptor(),
        sessionViewDescriptor('codex', 'alpha'),
        sessionViewDescriptor('pi', 'beta'),
      ],
      activeViewKey: 'session:codex:alpha',
      labelForView: (view: { kind: string }) => `tab-${view.kind}`,
    },
  }).body
}

describe('workspace tab strip layout', () => {
  it('renders one tab per view', () => {
    const body = strip()
    expect(body.match(/data-workspace-tab-item/g)).toHaveLength(3)
    expect(body).toContain('tab-inbox')
    expect(body).toContain('tab-session')
  })

  it('uses one measured width per tab and scrolls only when it has to', () => {
    const body = strip()
    // Tabs share the strip through an inline width; the strip owns the scrolling.
    expect(body).toMatch(/style="width: \d+px;"/)
    expect(body).toContain('shrink-0')
    // Unmeasured strip is not overflowing: overflow-x-auto would make a one-tab
    // strip autoscroll on middle-click, so it is applied only after layout says so.
    expect(body).toContain('data-overflowing="false"')
    expect(body).not.toMatch(/workspace-tab-list[^>]*overflow-x-auto/)
    expect(body).toContain('--tab-fade-start')
    expect(body).toContain('--tab-fade-end')
  })
})

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

  it('shrinks tabs down to a minimum before the strip scrolls', () => {
    const body = strip()
    // basis-48 is the preferred width, min-w-28 the floor, and shrink must stay on.
    expect(body).toMatch(/class="[^"]*basis-48[^"]*"/)
    expect(body).toMatch(/class="[^"]*min-w-28[^"]*"/)
    expect(body).not.toMatch(/class="[^"]*shrink-0[^"]*basis-48/)
    expect(body).toContain('overflow-x-auto')
  })
})

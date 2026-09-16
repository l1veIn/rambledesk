// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  createUnavailableWorkbenchCapabilities,
  UNAVAILABLE_CAPABILITY_MANIFEST,
} from '$lib/capabilities/unavailableCapabilities'
import { createWorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
import ChangedFilesCardHarness from './changedFilesCardHarness.svelte'
import type { ChangedFile } from './changed-files'

const files: ChangedFile[] = [
  { id: '/repo/src/main.ts', path: '/repo/src/main.ts', kind: 'modified', additions: 3, deletions: 1, diff: '--- a/main.ts' },
  { id: '/repo/old.ts', path: '/repo/old.ts', kind: 'removed', additions: 0, deletions: 4, diff: '' },
]

let app: ReturnType<typeof mount> | undefined
afterEach(() => { if (app) void unmount(app); app = undefined })

function capabilities(reveal: (path: string) => Promise<void>) {
  const unavailable = createUnavailableWorkbenchCapabilities()
  return createWorkbenchCapabilities({
    ...unavailable,
    serverPaths: {
      status: { availability: 'available', source: 'native' },
      implementation: { ...unavailable.serverPaths.implementation, reveal },
    },
  })
}

function target(reveal: (path: string) => Promise<void>) {
  const host = document.createElement('div')
  document.body.append(host)
  app = mount(ChangedFilesCardHarness, { target: host, props: { capabilities: capabilities(reveal), files, cwd: '/repo', onOpenDiff: vi.fn() } })
  return host
}

describe('changed-files reveal action', () => {
  it('reveals the agent-reported path and reports a failure inline', async () => {
    const reveal = vi.fn(async () => { throw new Error('no file manager') })
    const host = target(reveal)
    await tick()
    const buttons = [...host.querySelectorAll<HTMLButtonElement>('button[aria-label^="Reveal in file manager"]')]
    expect(buttons).toHaveLength(1) // The removed file has nothing to reveal.
    buttons[0]!.click()
    await vi.waitFor(() => expect(reveal).toHaveBeenCalledWith('/repo/src/main.ts'))
    await vi.waitFor(() => expect(host.textContent).toContain('Could not reveal the file.'))
  })

  it('hides the reveal control when the desktop capability is unavailable', async () => {
    const host = document.createElement('div')
    document.body.append(host)
    const unavailable = createUnavailableWorkbenchCapabilities()
    expect(unavailable.manifest.serverPaths.availability).toBe(UNAVAILABLE_CAPABILITY_MANIFEST.serverPaths.availability)
    app = mount(ChangedFilesCardHarness, { target: host, props: { capabilities: unavailable, files, onOpenDiff: vi.fn() } })
    await tick()
    expect(host.querySelectorAll('button[aria-label^="Reveal in file manager"]')).toHaveLength(0)
    expect(host.textContent).toContain('main.ts')
  })

  it('collapses to the summary row and keeps the removed file static', async () => {
    const host = target(vi.fn(async () => {}))
    await tick()
    expect(host.textContent).toContain('2 files')
    expect(host.textContent).toContain('Removed')
    host.querySelector<HTMLButtonElement>('section > button')!.click()
    await tick()
    expect(host.querySelector('[aria-label^="View changes"]')).toBeNull()
  })
})

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { render } from 'svelte/server'

vi.hoisted(() => {
  // The pending-speech queue restores from localStorage when the module loads.
  localStorage.setItem(
    'rambledesk.speech.pending-drafts',
    JSON.stringify([
      {
        id: 'draft-1',
        requestId: 'request-1',
        requestTitle: 'Review the shell',
        text: 'First spoken segment',
        status: 'pending',
        error: '',
        action: null,
      },
    ]),
  )
})

import { speechOverlayEnabled } from '../preferences'
import RambleSessionController from './RambleSessionController.svelte'
import { createUnavailableWorkbenchCapabilities } from '../capabilities/unavailableCapabilities'
import type { WorkbenchCapabilities } from '../capabilities/workbenchCapabilities'

const unavailable = createUnavailableWorkbenchCapabilities()

type RambleCapabilities = Pick<
  WorkbenchCapabilities,
  'screenCapture' | 'clipboardCapture' | 'globalShortcuts' | 'speech' | 'rambleConsole'
>

function capabilities(rambleConsoleAvailable = false): RambleCapabilities {
  return {
    screenCapture: unavailable.screenCapture,
    clipboardCapture: unavailable.clipboardCapture,
    globalShortcuts: unavailable.globalShortcuts,
    speech: unavailable.speech,
    rambleConsole: rambleConsoleAvailable
      ? {
          status: { availability: 'available', source: 'native' },
          implementation: {
            ...unavailable.rambleConsole.implementation,
            publishSpeechOverlay: vi.fn(async () => undefined),
            publish: vi.fn(async () => undefined),
          },
        }
      : unavailable.rambleConsole,
  }
}

function controller(extra: Record<string, unknown> = {}) {
  return render(RambleSessionController, {
    props: {
      capabilities: capabilities(),
      workspace: null,
      ...extra,
    },
  }).body
}

beforeEach(() => {
  speechOverlayEnabled.set(true)
})

describe('Ramble session controller rendering', () => {
  it('renders the native overlay host when the Ramble console is unavailable', () => {
    const html = controller()
    expect(html).toContain('speech-capsule-host')
    expect(html).toContain('--speech-overlay-opacity')
  })

  it('skips the overlay host when the native console is available', () => {
    const html = controller({ capabilities: capabilities(true) })
    expect(html).not.toContain('speech-capsule-host')
  })

  it('renders the review dock for pending speech while the overlay is disabled', () => {
    speechOverlayEnabled.set(false)
    const html = controller()
    expect(html).toContain('speech-review-dock')
    expect(html).toContain('Pending speech · 1')
  })

  it('keeps the dock collapsed until the human expands it', () => {
    speechOverlayEnabled.set(false)
    const html = controller()
    expect(html).toContain('aria-expanded="false"')
    expect(html).toContain('aria-controls="pending-speech-review"')
    expect(html).not.toContain('id="pending-speech-review"')
  })

  it('drops the dock when the pending queue is empty', () => {
    speechOverlayEnabled.set(false)
    localStorage.setItem('rambledesk.speech.pending-drafts', '[]')
    const html = controller()
    expect(html).not.toContain('speech-review-dock')
  })
})

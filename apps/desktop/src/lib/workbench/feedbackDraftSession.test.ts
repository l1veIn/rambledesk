import { describe, expect, it, vi } from 'vitest'

import { createActiveRambleCoordinator } from './activeRambleCoordinator'
import { createDraftSessionHost } from './draftSessionHost'
import { createFeedbackDraftSession } from './feedbackDraftSession'
import type { FeedbackEditorHandle } from './types'

function memorySave() {
  const bodies = new Map<string, { body: string; revision: number }>()
  return {
    bodies,
    save: vi.fn(async (input: { requestId: string; body: string; expectedRevision: number }) => {
      const current = bodies.get(input.requestId)
      if (current && current.revision !== input.expectedRevision) {
        throw new Error('revision conflict')
      }
      const savedRevision = input.expectedRevision + 1
      bodies.set(input.requestId, { body: input.body, revision: savedRevision })
      return { savedRevision }
    }),
  }
}

describe('FeedbackDraftSession', () => {
  it('appends speech into its own body and saves against its request id', async () => {
    const port = memorySave()
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialMarkdown: 'Hello',
      initialRevision: 3,
      save: port,
    })
    session.appendSpeech('First stable')
    expect(session.markdown()).toBe('Hello\n\nFirst stable')
    await expect(session.saveNow()).resolves.toBe(true)
    expect(port.save).toHaveBeenCalledWith({
      requestId: 'request-a',
      body: 'Hello\n\nFirst stable',
      expectedRevision: 3,
    })
    expect(session.savedRevision()).toBe(4)
  })

  it('ignores save results after dispose / generation invalidation', async () => {
    let finish: (value: { savedRevision: number }) => void = () => {}
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialMarkdown: 'Hello',
      initialRevision: 1,
      save: {
        save: () =>
          new Promise((resolve) => {
            finish = resolve
          }),
      },
    })
    session.appendSpeech('Pending')
    const pending = session.saveNow()
    session.dispose()
    finish({ savedRevision: 99 })
    await expect(pending).resolves.toBe(false)
    expect(session.markdown()).toBe('Hello\n\nPending')
    expect(session.savedRevision()).toBe(1)
  })

  it('routes speech through a bound editor instead of rewriting markdown itself', () => {
    const appendTranscript = vi.fn()
    const editor = {
      appendTranscript,
      applyExternalMarkdown: vi.fn(),
    } as unknown as FeedbackEditorHandle
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialMarkdown: 'Hello',
      initialRevision: 1,
      save: memorySave(),
    })
    session.bindEditor(editor)
    session.appendSpeech('Spoken')
    expect(appendTranscript).toHaveBeenCalledWith('Spoken')
    expect(session.markdown()).toBe('Hello')
    session.applyUserEdit('Hello\n\nSpoken')
    expect(session.markdown()).toBe('Hello\n\nSpoken')
  })
})

describe('DraftSessionHost', () => {
  it('reuses the owner session when returning from another visible request', async () => {
    const port = memorySave()
    const host = createDraftSessionHost({ save: port })
    const a = host.openVisible({ requestId: 'a', markdown: 'Draft A', revision: 1 })
    host.setOwner('a')
    a.appendSpeech('Keep me')
    host.openVisible({ requestId: 'b', markdown: 'Draft B', revision: 8 })
    expect(host.visible()?.requestId).toBe('b')
    expect(host.owner()?.markdown()).toBe('Draft A\n\nKeep me')
    const returned = host.openVisible({ requestId: 'a', markdown: 'STALE FROM SQLITE', revision: 99 })
    expect(returned).toBe(a)
    expect(returned.markdown()).toBe('Draft A\n\nKeep me')
    expect(returned.savedRevision()).toBe(1)
  })

  it('never mounts more than the owner and the visible request', () => {
    const host = createDraftSessionHost({ save: memorySave() })
    host.openVisible({ requestId: 'a', markdown: 'A', revision: 1 })
    host.setOwner('a')
    host.openVisible({ requestId: 'b', markdown: 'B', revision: 1 })
    host.openVisible({ requestId: 'c', markdown: 'C', revision: 1 })
    const ids = host.mounted().map((session) => session.requestId)
    expect(ids).toEqual(['a', 'c'])
    expect(host.get('b')?.isDisposed() ?? true).toBe(true)
  })

  it('drops stale sessions when generation bumps', () => {
    const host = createDraftSessionHost({ save: memorySave() })
    const first = host.openVisible({ requestId: 'a', markdown: 'A', revision: 1 })
    expect(host.currentGeneration()).toBe(1)
    host.bumpGeneration()
    expect(first.isDisposed()).toBe(true)
    expect(host.mounted()).toEqual([])
    expect(host.currentGeneration()).toBe(2)
  })
})

describe('ActiveRambleCoordinator', () => {
  it('requires an explicit handoff when starting on a different request', () => {
    const ramble = createActiveRambleCoordinator()
    ramble.occupy('a')
    expect(ramble.needsHandoff('b')).toBe(true)
    expect(ramble.needsHandoff('a')).toBe(false)
    ramble.release()
    expect(ramble.needsHandoff('b')).toBe(false)
  })
})

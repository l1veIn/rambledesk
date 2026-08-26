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

  it('toggles the current Action channel without pasting the instruction', () => {
    const setActionChannel = vi.fn()
    const appendTranscript = vi.fn()
    const editor = {
      appendTranscript,
      applyExternalMarkdown: vi.fn(),
      setActionChannel,
    } as unknown as FeedbackEditorHandle
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialMarkdown: 'Hello',
      initialRevision: 1,
      save: memorySave(),
    })
    session.bindEditor(editor)
    session.toggleActionChannel(2)
    expect(session.currentActionIndex()).toBe(2)
    expect(setActionChannel).toHaveBeenCalledWith(2)
    session.appendSpeech('保存之后没有 toast。')
    expect(appendTranscript).toHaveBeenCalledWith('保存之后没有 toast。')
    expect(session.markdown()).toBe('Hello')
    session.toggleActionChannel(2)
    expect(session.currentActionIndex()).toBeNull()
    expect(setActionChannel).toHaveBeenCalledWith(null)
  })

  it('tags speech with the current Action when no editor is bound', () => {
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialMarkdown: 'Hello',
      initialRevision: 1,
      save: memorySave(),
    })
    session.toggleActionChannel(2)
    session.appendSpeech('保存之后没有 toast。')
    expect(session.markdown()).toBe('Hello\n\n@ Action 2\n\n保存之后没有 toast。')
    session.toggleActionChannel(2)
    session.appendSpeech('其实 toast 在右下角。')
    expect(session.markdown()).toBe(
      'Hello\n\n@ Action 2\n\n保存之后没有 toast。\n\n@\n\n其实 toast 在右下角。',
    )
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

describe('Light cleanup on a draft session', () => {
  it('replaces three pending stables with the cleaned text', async () => {
    const clean = vi.fn(async (text: string) => `cleaned:${text}`)
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialMarkdown: 'Hello',
      initialRevision: 1,
      save: memorySave(),
      cleanup: { enabled: () => true, clean, silenceMs: 60_000, timeoutMs: 5_000 },
    })
    session.appendSpeech('one')
    session.appendSpeech('two')
    expect(clean).not.toHaveBeenCalled()
    session.appendSpeech('three')
    await session.settle()
    expect(clean).toHaveBeenCalledWith('one\n\ntwo\n\nthree')
    expect(session.markdown()).toContain('cleaned:')
    expect(session.markdown()).toContain('one')
    expect(session.isCleaning()).toBe(false)
  })

  it('keeps the raw text when cleanup times out', async () => {
    const queued: Array<() => void> = []
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialMarkdown: 'Hello',
      initialRevision: 1,
      save: memorySave(),
      cleanup: {
        enabled: () => true,
        clean: () => new Promise(() => {}),
        silenceMs: 60_000,
        timeoutMs: 10,
        schedule: (fn, ms) => {
          if (ms === 10) queued.push(fn)
          return queued.length
        },
        cancel: () => {},
      },
    })
    session.appendSpeech('one')
    session.appendSpeech('two')
    session.appendSpeech('three')
    queued.forEach((fn) => fn())
    await session.settle()
    expect(session.markdown()).toBe('Hello\n\none\n\ntwo\n\nthree')
  })

  it('does not duplicate speech when a clipboard block is inserted before cleanup finishes', async () => {
    const clean = vi.fn(async (text: string) => text.replace(/啊。?$/, '。'))
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialMarkdown: '',
      initialRevision: 1,
      save: memorySave(),
      cleanup: { enabled: () => true, clean, silenceMs: 60_000, timeoutMs: 5_000 },
    })
    session.appendSpeech('我试一下复制粘贴啊。')
    session.insertMarkdownBlock('> Clipboard import')
    await session.settle()
    const spoken = session.markdown().match(/复制粘贴/g) ?? []
    expect(spoken).toHaveLength(1)
    expect(session.markdown()).toContain('Clipboard import')
  })

  it('does not apply a cleanup result after dispose', async () => {
    let finish = (_text: string) => {}
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialMarkdown: 'Hello',
      initialRevision: 1,
      save: memorySave(),
      cleanup: {
        enabled: () => true,
        clean: () =>
          new Promise((resolve) => {
            finish = resolve
          }),
        silenceMs: 60_000,
        timeoutMs: 5_000,
      },
    })
    session.appendSpeech('one')
    session.appendSpeech('two')
    session.appendSpeech('three')
    await Promise.resolve()
    session.dispose()
    finish('GONE')
    await Promise.resolve()
    expect(session.markdown()).toBe('Hello\n\none\n\ntwo\n\nthree')
  })

  it('hands the cleaned text to the editor so it can mark rewritten sentences', async () => {
    const finishSpeechCleanup = vi.fn()
    const editor = {
      appendTranscript: vi.fn(),
      applyExternalMarkdown: vi.fn(),
      beginSpeechCleanup: vi.fn(() => '啊那个按钮太小了'),
      finishSpeechCleanup,
      isSpeechCleaning: vi.fn(() => true),
      moveCursorAfterCleaningSpeech: vi.fn(),
    } as unknown as FeedbackEditorHandle
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialMarkdown: '',
      initialRevision: 1,
      save: memorySave(),
      cleanup: {
        enabled: () => true,
        clean: async () => '按钮太小了。',
        silenceMs: 60_000,
        timeoutMs: 5_000,
      },
    })
    session.bindEditor(editor)
    session.appendSpeech('啊那个按钮太小了')
    session.appendSpeech('第二句')
    session.appendSpeech('第三句')
    await session.settle()
    expect(finishSpeechCleanup).toHaveBeenCalledWith('按钮太小了。')
  })

  it('still marks speech cleaned when the model returns the same wording', async () => {
    const finishSpeechCleanup = vi.fn()
    const editor = {
      appendTranscript: vi.fn(),
      applyExternalMarkdown: vi.fn(),
      beginSpeechCleanup: vi.fn(() => '按钮太小了。'),
      finishSpeechCleanup,
      isSpeechCleaning: vi.fn(() => true),
      moveCursorAfterCleaningSpeech: vi.fn(),
    } as unknown as FeedbackEditorHandle
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialMarkdown: '',
      initialRevision: 1,
      save: memorySave(),
      cleanup: {
        enabled: () => true,
        clean: async (text: string) => text,
        silenceMs: 60_000,
        timeoutMs: 5_000,
      },
    })
    session.bindEditor(editor)
    session.appendSpeech('按钮太小了。')
    session.appendSpeech('第二句')
    session.appendSpeech('第三句')
    await session.settle()
    expect(finishSpeechCleanup).toHaveBeenCalledWith('按钮太小了。')
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

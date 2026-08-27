import { describe, expect, it, vi } from 'vitest'

import {
  decodeFeedbackDraftDocument,
  snapshotFeedbackDraftDocument,
  snapshotFeedbackDraftMarkdown,
  updateFeedbackDraftDocument,
} from '../feedbackDraftDocument'
import {
  CLEANUP_STATE_ATTR,
  INPUT_SOURCE_ATTR,
  SPEECH_SEGMENT_ID_ATTR,
  asrParagraphAttrs,
} from '../speechBlockMetadata'
import { createActiveRambleCoordinator } from './activeRambleCoordinator'
import { actionChannelFor } from './actionChannelState'
import { createDraftSessionHost } from './draftSessionHost'
import { createFeedbackDraftSession } from './feedbackDraftSession'
import type { FeedbackEditorHandle } from './types'

function memorySave() {
  const bodies = new Map<
    string,
    { documentJson: string; bodyMarkdown: string; revision: number }
  >()
  return {
    bodies,
    save: vi.fn(async (input: {
      requestId: string
      documentJson: string
      bodyMarkdown: string
      expectedRevision: number
    }) => {
      const current = bodies.get(input.requestId)
      if (current && current.revision !== input.expectedRevision) {
        throw new Error('revision conflict')
      }
      const savedRevision = input.expectedRevision + 1
      bodies.set(input.requestId, {
        documentJson: input.documentJson,
        bodyMarkdown: input.bodyMarkdown,
        revision: savedRevision,
      })
      return { savedRevision }
    }),
  }
}

describe('FeedbackDraftSession', () => {
  it('restores the saved TipTap document instead of reconstructing from Markdown', async () => {
    const port = memorySave()
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialMarkdown: '',
      initialRevision: 0,
      save: port,
    })
    session.applyUserEdit(
      snapshotFeedbackDraftDocument({
        type: 'doc',
        content: [
          {
            type: 'paragraph',
            attrs: { ...asrParagraphAttrs('segment-1', 'pending'), actionIndex: 2 },
            content: [{ type: 'text', text: '还没有整理' }],
          },
        ],
      }),
    )
    await expect(session.saveNow()).resolves.toBe(true)
    const stored = port.bodies.get('request-a')!

    const restarted = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 2,
      initialDocumentJson: stored.documentJson,
      initialMarkdown: stored.bodyMarkdown,
      initialRevision: stored.revision,
      save: port,
    })

    expect(restarted.documentJson()).toBe(stored.documentJson)
    expect(decodeFeedbackDraftDocument(restarted.documentJson())?.content?.[0]).toMatchObject({
      type: 'paragraph',
      attrs: {
        [SPEECH_SEGMENT_ID_ATTR]: 'segment-1',
        [INPUT_SOURCE_ATTR]: 'asr',
        [CLEANUP_STATE_ATTR]: 'pending',
        actionIndex: 2,
      },
    })
  })

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
    expect(decodeFeedbackDraftDocument(session.documentJson())?.content?.[1].attrs).toMatchObject({
      [INPUT_SOURCE_ATTR]: 'asr',
      [CLEANUP_STATE_ATTR]: 'skipped',
      [SPEECH_SEGMENT_ID_ATTR]: expect.any(String),
    })
    await expect(session.saveNow()).resolves.toBe(true)
    expect(port.save).toHaveBeenCalledWith({
      requestId: 'request-a',
      documentJson: expect.any(String),
      bodyMarkdown: 'Hello\n\nFirst stable',
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
    session.beginRecording()
    session.appendSpeech('Spoken')
    expect(appendTranscript).toHaveBeenCalledWith('Spoken', {
      asr: { cleanupState: 'skipped', segmentId: expect.any(String) },
    })
    expect(session.markdown()).toBe('Hello')
    session.applyUserEdit(snapshotFeedbackDraftMarkdown('Hello\n\nSpoken'))
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
    session.beginRecording()
    session.toggleActionChannel(2)
    expect(session.currentActionIndex()).toBe(2)
    expect(setActionChannel).toHaveBeenCalledWith(2)
    session.appendSpeech('保存之后没有 toast。')
    expect(appendTranscript).toHaveBeenCalledWith('保存之后没有 toast。', {
      asr: { cleanupState: 'skipped', segmentId: expect.any(String) },
    })
    expect(session.markdown()).toBe('Hello')
    session.toggleActionChannel(2)
    expect(session.currentActionIndex()).toBeNull()
    expect(setActionChannel).toHaveBeenCalledWith(null)
    expect(actionChannelFor('request-a')).toBeNull()
  })

  it('shares the channel with editors through the single store and clears it on demand', () => {
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialMarkdown: 'Hello',
      initialRevision: 1,
      save: memorySave(),
    })
    expect(actionChannelFor('request-a')).toBeNull()
    session.toggleActionChannel(2)
    expect(actionChannelFor('request-a')).toBe(2)
    session.clearActionChannel()
    expect(session.currentActionIndex()).toBeNull()
    expect(actionChannelFor('request-a')).toBeNull()
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
    expect(session.markdown()).toBe(
      'Hello\n\n------------------------ Action 2 ------------------------\n\n保存之后没有 toast。',
    )
    session.toggleActionChannel(2)
    session.appendSpeech('其实 toast 在右下角。')
    expect(session.markdown()).toBe(
      'Hello\n\n------------------------ Action 2 ------------------------\n\n保存之后没有 toast。\n\n------------------------------------------------\n\n其实 toast 在右下角。',
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
      cleanup: {
        enabled: () => true,
        clean,
        settings: () => ({ segmentThreshold: 3, charThreshold: 500, idleMs: 60_000, timeoutMs: 5_000 }),
      },
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
    expect(
      decodeFeedbackDraftDocument(session.documentJson())?.content
        ?.slice(-3)
        .map((node) => node.attrs?.[CLEANUP_STATE_ATTR]),
    ).toEqual(['cleaned', 'cleaned', 'cleaned'])
  })

  it('starts after the configured idle period when pending ASR nodes remain', async () => {
    let runIdle = () => {}
    const clean = vi.fn(async (text: string) => text.replace('啊', ''))
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialMarkdown: '',
      initialRevision: 1,
      save: memorySave(),
      cleanup: {
        enabled: () => true,
        clean,
        settings: () => ({ segmentThreshold: 3, charThreshold: 500, idleMs: 10_000, timeoutMs: 30_000 }),
        schedule: (fn, ms) => {
          if (ms === 10_000) runIdle = fn
          return ms
        },
        cancel: () => {},
      },
    })

    session.appendSpeech('啊按钮太小了')
    expect(clean).not.toHaveBeenCalled()
    runIdle()
    await session.settle()
    expect(clean).toHaveBeenCalledOnce()
    expect(session.markdown()).toBe('按钮太小了')
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
        settings: () => ({ segmentThreshold: 3, charThreshold: 500, idleMs: 60_000, timeoutMs: 10 }),
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
    expect(
      decodeFeedbackDraftDocument(session.documentJson())?.content
        ?.slice(-3)
        .map((node) => node.attrs?.[CLEANUP_STATE_ATTR]),
    ).toEqual(['failed', 'failed', 'failed'])
  })

  it('does not overwrite an ASR node changed after its cleanup batch started', async () => {
    let finish = (_text: string) => {}
    let nextId = 0
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialMarkdown: '',
      initialRevision: 1,
      save: memorySave(),
      createSpeechSegmentId: () => `segment-${++nextId}`,
      cleanup: {
        enabled: () => true,
        clean: () => new Promise((resolve) => (finish = resolve)),
        settings: () => ({ segmentThreshold: 3, charThreshold: 500, idleMs: 60_000, timeoutMs: 30_000 }),
      },
    })
    session.appendSpeech('one')
    session.appendSpeech('two')
    session.appendSpeech('three')
    await Promise.resolve()
    session.applyUserEdit(
      updateFeedbackDraftDocument(session.snapshot(), (doc) => ({
        ...doc,
        content: (doc.content ?? []).map((node) =>
          node.attrs?.[SPEECH_SEGMENT_ID_ATTR] === 'segment-1'
            ? { ...node, content: [{ type: 'text', text: 'human edit' }] }
            : node,
        ),
      })),
    )
    finish('clean one\n\nclean two\n\nclean three')
    await session.settle()

    const nodes = decodeFeedbackDraftDocument(session.documentJson())?.content ?? []
    expect(nodes[0]).toMatchObject({
      attrs: { [CLEANUP_STATE_ATTR]: 'skipped' },
      content: [{ text: 'human edit' }],
    })
    expect(nodes.slice(1).map((node) => node.attrs?.[CLEANUP_STATE_ATTR])).toEqual([
      'cleaned',
      'cleaned',
    ])
  })

  it('does not duplicate speech when a clipboard block is inserted before cleanup finishes', async () => {
    const clean = vi.fn(async (text: string) => text.replace(/啊。?$/, '。'))
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialMarkdown: '',
      initialRevision: 1,
      save: memorySave(),
      cleanup: {
        enabled: () => true,
        clean,
        settings: () => ({ segmentThreshold: 3, charThreshold: 500, idleMs: 60_000, timeoutMs: 5_000 }),
      },
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
        settings: () => ({ segmentThreshold: 3, charThreshold: 500, idleMs: 60_000, timeoutMs: 5_000 }),
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
    const pending = snapshotFeedbackDraftDocument({
      type: 'doc',
      content: ['啊那个按钮太小了', '第二句', '第三句'].map((text, index) => ({
        type: 'paragraph',
        attrs: asrParagraphAttrs(`segment-${index + 1}`, 'pending'),
        content: [{ type: 'text', text }],
      })),
    })
    const finishSpeechCleanup = vi.fn()
    const editor = {
      appendTranscript: vi.fn(),
      applyExternalMarkdown: vi.fn(),
      beginSpeechCleanup: vi.fn(),
      finishSpeechCleanup,
      isSpeechCleaning: vi.fn(() => true),
      moveCursorAfterCleaningSpeech: vi.fn(),
    } as unknown as FeedbackEditorHandle
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialDocumentJson: pending.documentJson,
      initialMarkdown: pending.bodyMarkdown,
      initialRevision: 1,
      save: memorySave(),
      cleanup: {
        enabled: () => true,
        clean: async () => '按钮太小了。',
        settings: () => ({ segmentThreshold: 3, charThreshold: 500, idleMs: 60_000, timeoutMs: 5_000 }),
      },
    })
    session.bindEditor(editor)
    session.beginRecording()
    await session.settle()
    expect(finishSpeechCleanup).toHaveBeenCalledWith(
      expect.arrayContaining([
        { segmentId: 'segment-1', text: '啊那个按钮太小了' },
      ]),
      '按钮太小了。',
    )
  })

  it('still marks speech cleaned when the model returns the same wording', async () => {
    const pending = snapshotFeedbackDraftDocument({
      type: 'doc',
      content: [
        {
          type: 'paragraph',
          attrs: asrParagraphAttrs('segment-1', 'pending'),
          content: [{ type: 'text', text: '按钮太小了。' }],
        },
      ],
    })
    const finishSpeechCleanup = vi.fn()
    const editor = {
      appendTranscript: vi.fn(),
      applyExternalMarkdown: vi.fn(),
      beginSpeechCleanup: vi.fn(),
      finishSpeechCleanup,
      isSpeechCleaning: vi.fn(() => true),
      moveCursorAfterCleaningSpeech: vi.fn(),
    } as unknown as FeedbackEditorHandle
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialDocumentJson: pending.documentJson,
      initialMarkdown: pending.bodyMarkdown,
      initialRevision: 1,
      save: memorySave(),
      cleanup: {
        enabled: () => true,
        clean: async (text: string) => text,
        settings: () => ({ segmentThreshold: 3, charThreshold: 500, idleMs: 60_000, timeoutMs: 5_000 }),
      },
    })
    session.bindEditor(editor)
    session.beginRecording()
    await session.settle()
    expect(finishSpeechCleanup).toHaveBeenCalledWith(
      [{ segmentId: 'segment-1', text: '按钮太小了。' }],
      '按钮太小了。',
    )
  })

  it('merges a model result that collapsed the batch into one block', async () => {
    const pending = snapshotFeedbackDraftDocument({
      type: 'doc',
      content: [
        {
          type: 'paragraph',
          attrs: asrParagraphAttrs('segment-1', 'pending'),
          content: [{ type: 'text', text: '第一句啊那个' }],
        },
        {
          type: 'paragraph',
          attrs: asrParagraphAttrs('segment-2', 'pending'),
          content: [{ type: 'text', text: '第二句嗯嗯' }],
        },
      ],
    })
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialDocumentJson: pending.documentJson,
      initialMarkdown: pending.bodyMarkdown,
      initialRevision: 1,
      save: memorySave(),
      cleanup: {
        enabled: () => true,
        clean: async () => '第一句，第二句。',
        settings: () => ({ segmentThreshold: 3, charThreshold: 500, idleMs: 60_000, timeoutMs: 5_000 }),
      },
    })
    session.bindEditor({} as FeedbackEditorHandle)
    await session.settle()
    const doc = decodeFeedbackDraftDocument(session.snapshot().documentJson)
    expect(doc?.content?.length).toBe(1)
    expect(doc?.content?.[0]?.content?.[0]?.text).toBe('第一句，第二句。')
    expect(doc?.content?.[0]?.attrs?.cleanupState).toBe('cleaned')
  })

  it('fills a batch whose model output kept one block per segment', async () => {
    const pending = snapshotFeedbackDraftDocument({
      type: 'doc',
      content: [
        {
          type: 'paragraph',
          attrs: asrParagraphAttrs('segment-1', 'pending'),
          content: [{ type: 'text', text: '第一句啊那个' }],
        },
        {
          type: 'paragraph',
          attrs: asrParagraphAttrs('segment-2', 'pending'),
          content: [{ type: 'text', text: '第二句嗯嗯' }],
        },
      ],
    })
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialDocumentJson: pending.documentJson,
      initialMarkdown: pending.bodyMarkdown,
      initialRevision: 1,
      save: memorySave(),
      cleanup: {
        enabled: () => true,
        clean: async () => '第一句。\n\n第二句。',
        settings: () => ({ segmentThreshold: 3, charThreshold: 500, idleMs: 60_000, timeoutMs: 5_000 }),
      },
    })
    session.bindEditor({} as FeedbackEditorHandle)
    await session.settle()
    const doc = decodeFeedbackDraftDocument(session.snapshot().documentJson)
    expect(doc?.content?.length).toBe(2)
    expect(doc?.content?.[0]?.content?.[0]?.text).toBe('第一句。')
    expect(doc?.content?.[1]?.content?.[0]?.text).toBe('第二句。')
  })

  it('tidies on demand with auto tidy off and refills by label', async () => {
    const pending = snapshotFeedbackDraftDocument({
      type: 'doc',
      content: [
        {
          type: 'paragraph',
          attrs: asrParagraphAttrs('segment-1', 'pending'),
          content: [{ type: 'text', text: 'Um first line' }],
        },
        {
          type: 'paragraph',
          attrs: asrParagraphAttrs('segment-2', 'pending'),
          content: [{ type: 'text', text: 'Err second line' }],
        },
      ],
    })
    const session = createFeedbackDraftSession({
      requestId: 'request-a',
      generation: 1,
      initialDocumentJson: pending.documentJson,
      initialMarkdown: pending.bodyMarkdown,
      initialRevision: 1,
      save: memorySave(),
      cleanup: {
        enabled: () => false,
        clean: async () => '[1] First line.\n[2] Second line.',
        settings: () => ({ segmentThreshold: 3, charThreshold: 500, idleMs: 60_000, timeoutMs: 5_000 }),
      },
    })
    session.bindEditor({} as FeedbackEditorHandle)
    expect(session.pendingCleanupCount()).toBe(2)
    expect(session.tidyNow()).toBe(true)
    await session.settle()
    const doc = decodeFeedbackDraftDocument(session.snapshot().documentJson)
    expect(doc?.content?.[0]?.content?.[0]?.text).toBe('First line.')
    expect(doc?.content?.[1]?.content?.[0]?.text).toBe('Second line.')
    expect(session.cleanupCount()).toBeGreaterThan(0)
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

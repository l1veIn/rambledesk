import { get } from 'svelte/store'
import { describe, expect, it, vi } from 'vitest'
import { createSpeechDraftQueue, groupSpeechDrafts, PENDING_SPEECH_KEY, type SpeechTarget } from './speechDraftQueue'

const target: SpeechTarget = { requestId: 'request-a', requestTitle: 'Request A', action: { actionId: 'action-a', actionIndex: 0, title: 'Action A' } }
const nextTarget: SpeechTarget = { requestId: 'request-b', requestTitle: 'Request B', action: null }
const deferred = () => {
  let resolve!: () => void
  const promise = new Promise<void>((done) => { resolve = done })
  return { promise, resolve }
}

describe('speech draft queue', () => {
  it('reports success only after the writer acknowledges the draft', async () => {
    const saved = deferred()
    const write = vi.fn(() => saved.promise)
    const queue = createSpeechDraftQueue({ write })
    queue.enqueue('segment-1', 'Hello', target, false)
    await Promise.resolve()
    expect(get(queue).drafts[0].status).toBe('writing')
    expect(get(queue).receipt).toBeNull()
    saved.resolve()
    await queue.settled()
    expect(get(queue).drafts).toEqual([])
    expect(get(queue).receipt).toMatchObject({ id: 'segment-1', text: 'Hello', requestId: 'request-a' })
  })

  it('keeps confirmed speech pinned and does not accept new arrivals with an old click', async () => {
    const saved = deferred()
    const write = vi.fn(() => saved.promise)
    const queue = createSpeechDraftQueue({ write })
    const mutable = { ...target, action: { ...target.action! } }
    queue.enqueue('one', 'First', mutable, true)
    mutable.action.actionId = 'different-action'
    expect(write).not.toHaveBeenCalled()
    const confirm = queue.accept(['one'])
    queue.enqueue('two', 'Second', nextTarget, true)
    void queue.accept(['one'])
    queue.discard(['one'])
    await Promise.resolve()
    expect(write).toHaveBeenCalledExactlyOnceWith('request-a', {
      kind: 'appendSpeech', segmentId: 'one', text: 'First', action: target.action,
    })
    saved.resolve()
    await confirm
    expect(get(queue).drafts.map((draft) => draft.id)).toEqual(['two'])
  })

  it('does not drain pending speech when later segments use direct writing', async () => {
    const write = vi.fn(async () => {})
    const queue = createSpeechDraftQueue({ write })
    queue.enqueue('pending', 'Review me', target, true)
    queue.enqueue('direct', 'Write me', nextTarget, false)
    await queue.settled()
    expect(write).toHaveBeenCalledTimes(1)
    expect(get(queue).drafts.map((draft) => draft.id)).toEqual(['pending'])
  })

  it('retains failed writes for an idempotent retry and ignores duplicate events', async () => {
    const write = vi.fn().mockRejectedValueOnce(new Error('Draft is temporarily unavailable')).mockResolvedValue(undefined)
    const queue = createSpeechDraftQueue({ write })
    queue.enqueue('one', 'Do not lose this', target, false)
    await queue.settled()
    expect(get(queue).receipt).toBeNull()
    expect(get(queue).drafts[0]).toMatchObject({ status: 'failed', error: 'Draft is temporarily unavailable' })
    await queue.accept(['one'])
    queue.enqueue('one', 'Duplicate', target, false)
    await queue.settled()
    expect(write).toHaveBeenCalledTimes(2)
    expect(write.mock.calls[0]).toEqual(write.mock.calls[1])
    expect(get(queue).drafts).toEqual([])
  })

  it('restores pending speech after reload without silently writing it', async () => {
    const values = new Map<string, string>()
    const storage = { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => { values.set(key, value) } }
    const first = createSpeechDraftQueue({ write: vi.fn(), storage })
    first.enqueue('one', 'Persist me', target, true)
    const write = vi.fn(async () => {})
    const restored = createSpeechDraftQueue({ write, storage })
    expect(restored.hasPending('request-a')).toBe(true)
    expect(write).not.toHaveBeenCalled()
    restored.discard(['one'])
    expect(JSON.parse(values.get(PENDING_SPEECH_KEY)!)).toEqual([])
    expect(createSpeechDraftQueue({ write, storage }).hasPending('request-a')).toBe(false)
  })

  it('groups continuous speech without merging different requests or actions', () => {
    const queue = createSpeechDraftQueue({ write: vi.fn() })
    queue.enqueue('one', 'First', target, true)
    queue.enqueue('two', 'Second', target, true)
    queue.enqueue('three', 'Third', nextTarget, true)
    queue.enqueue('four', 'Fourth', target, true)
    const groups = groupSpeechDrafts(get(queue).drafts)
    expect(groups.map((group) => group.ids)).toEqual([['one', 'two'], ['three'], ['four']])
    expect(groups[0].text).toBe('First\nSecond')
  })
})

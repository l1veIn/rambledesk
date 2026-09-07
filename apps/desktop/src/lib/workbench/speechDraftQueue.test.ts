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

const deferredText = () => {
  let resolve!: (text: string) => void
  let reject!: (cause: unknown) => void
  const promise = new Promise<string>((done, fail) => { resolve = done; reject = fail })
  return { promise, resolve, reject }
}

const memoryStorage = () => {
  const values = new Map<string, string>()
  return { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => { values.set(key, value) } }
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

  it('edits a frozen batch without accepting it or changing later speech', async () => {
    const storage = memoryStorage()
    const writes: string[] = []
    const queue = createSpeechDraftQueue({ storage, write: async (_request, operation) => { if (operation.kind === 'appendSpeech') writes.push(operation.text) } })
    queue.enqueue('one', 'First', target, true)
    queue.enqueue('two', 'Second', target, true)
    queue.beginEdit(['one', 'two'])
    expect(get(queue).edit).toEqual({ ids: ['one', 'two'], text: 'First\nSecond' })
    queue.enqueue('three', 'Later', target, true)
    expect(groupSpeechDrafts(get(queue).drafts).map(({ ids, busy, editing }) => ({ ids, busy, editing }))).toEqual([
      { ids: ['one', 'two'], busy: true, editing: true }, { ids: ['three'], busy: false, editing: false },
    ])
    await queue.accept(['one', 'two'])
    queue.discard(['one', 'two'])
    expect(writes).toEqual([])
    queue.saveEdit(['one', 'two'], '  Corrected text  ')
    expect(get(queue).edit).toBeNull()
    expect(get(queue).drafts.map(({ id, text, cleanupState }) => ({ id, text, cleanupState }))).toEqual([
      { id: 'one', text: 'Corrected text', cleanupState: 'pending' }, { id: 'three', text: 'Later', cleanupState: 'pending' },
    ])
    queue.enqueue('two', 'A delayed duplicate', target, true)
    expect(get(queue).drafts).toHaveLength(2)
    const restored = createSpeechDraftQueue({ storage, write: async () => {} })
    restored.enqueue('two', 'Duplicate after reload', target, true)
    expect(get(restored).edit).toBeNull()
    expect(get(restored).drafts.map(({ text }) => text)).toEqual(['Corrected text', 'Later'])
    await queue.accept(['one'])
    expect(writes).toEqual(['Corrected text'])
  })

  it('rejects edit selections with missing, repeated, reversed, noncontiguous, or mixed-target IDs', () => {
    const queue = createSpeechDraftQueue({ write: async () => {} })
    queue.enqueue('one', 'First', target, true)
    queue.enqueue('two', 'Second', target, true)
    queue.enqueue('three', 'Third', nextTarget, true)
    queue.enqueue('four', 'Fourth', { ...target, action: null }, true)
    for (const ids of [[], ['missing'], ['one', 'missing'], ['one', 'one'], ['two', 'one'], ['one', 'three'], ['two', 'three'], ['three', 'four']]) {
      queue.beginEdit(ids)
      expect(get(queue).edit).toBeNull()
    }
    queue.beginEdit(['two'])
    expect(get(queue).edit?.ids).toEqual(['two'])
    queue.beginEdit(['four'])
    expect(get(queue).edit?.ids).toEqual(['two'])
  })

  it('requires the exact edit snapshot and nonempty text before saving', () => {
    const queue = createSpeechDraftQueue({ write: async () => {} })
    queue.enqueue('one', 'First', target, true)
    queue.enqueue('two', 'Second', target, true)
    queue.beginEdit(['one', 'two'])
    queue.saveEdit(['one'], 'Wrong selection')
    queue.saveEdit(['two', 'one'], 'Wrong order')
    queue.saveEdit(['one', 'two'], ' \n ')
    queue.cancelEdit(['one'])
    expect(get(queue).edit).toEqual({ ids: ['one', 'two'], text: 'First\nSecond' })
    expect(get(queue).drafts.map(({ text }) => text)).toEqual(['First', 'Second'])
    queue.cancelEdit(['one', 'two'])
    expect(get(queue).edit).toBeNull()
    expect(get(queue).drafts.map(({ text, status }) => ({ text, status }))).toEqual([
      { text: 'First', status: 'pending' }, { text: 'Second', status: 'pending' },
    ])
  })

  it('tidies the captured text and keeps new arrivals outside its result', async () => {
    const response = deferredText()
    const inputs: string[] = []
    const queue = createSpeechDraftQueue({ write: async () => {}, tidy: (text) => { inputs.push(text); return response.promise } })
    queue.enqueue('one', 'first um', target, true)
    queue.enqueue('two', 'second', target, true)
    const tidy = queue.tidy(['one', 'two'])
    queue.enqueue('three', 'New words', target, true)
    expect(inputs).toEqual(['first um\nsecond'])
    expect(groupSpeechDrafts(get(queue).drafts).map(({ ids, busy, tidying }) => ({ ids, busy, tidying }))).toEqual([
      { ids: ['one', 'two'], busy: true, tidying: true }, { ids: ['three'], busy: false, tidying: false },
    ])
    queue.beginEdit(['one'])
    await queue.accept(['one', 'two'])
    expect(get(queue).edit).toBeNull()
    response.resolve('  First. Second.  ')
    await tidy
    expect(get(queue).drafts.map(({ id, text, cleanupState }) => ({ id, text, cleanupState }))).toEqual([
      { id: 'one', text: 'First. Second.', cleanupState: 'cleaned' }, { id: 'three', text: 'New words', cleanupState: 'pending' },
    ])
    expect(get(queue).receipt).toBeNull()
    queue.enqueue('two', 'Duplicate', target, true)
    expect(get(queue).drafts).toHaveLength(2)
  })

  it('rejects invalid tidy selections without sending their text to cleanup', async () => {
    const inputs: string[] = []
    const queue = createSpeechDraftQueue({ write: async () => {}, tidy: async (text) => { inputs.push(text); return 'Cleaned' } })
    queue.enqueue('one', 'First', target, true)
    queue.enqueue('two', 'Second', target, true)
    queue.enqueue('three', 'Other target', nextTarget, true)
    for (const ids of [[], ['missing'], ['one', 'missing'], ['one', 'one'], ['two', 'one'], ['one', 'three'], ['two', 'three']]) await queue.tidy(ids)
    queue.beginEdit(['two'])
    await queue.tidy(['two'])
    expect(inputs).toEqual([])
    expect(get(queue).drafts.map(({ text }) => text)).toEqual(['First', 'Second', 'Other target'])
  })

  it.each(['empty', 'invalid', 'rejected'] as const)('keeps original text after %s cleanup with a visible retryable error', async (outcome) => {
    let attempts = 0
    const queue = createSpeechDraftQueue({ write: async () => {}, tidy: async () => {
      attempts += 1
      if (attempts > 1) return 'Cleaned text'
      if (outcome === 'rejected') throw new Error('Provider unavailable')
      return outcome === 'empty' ? ' \n ' : null as unknown as string
    } })
    queue.enqueue('one', 'Original one', target, true)
    queue.enqueue('two', 'Original two', target, true)
    await queue.tidy(['one', 'two'])
    expect(get(queue).drafts.map(({ id, text, status }) => ({ id, text, status }))).toEqual([
      { id: 'one', text: 'Original one', status: 'pending' }, { id: 'two', text: 'Original two', status: 'pending' },
    ])
    expect(groupSpeechDrafts(get(queue).drafts)[0]).toMatchObject({ busy: false, editable: true, cleanupState: 'pending' })
    expect(groupSpeechDrafts(get(queue).drafts)[0].error).not.toBe('')
    expect(get(queue).receipt).toBeNull()
    await queue.tidy(['one', 'two'])
    expect(get(queue).drafts[0]).toMatchObject({ text: 'Cleaned text', error: '', cleanupState: 'cleaned' })
  })

  it('ignores a late cleanup result after its batch was discarded', async () => {
    const response = deferredText()
    const queue = createSpeechDraftQueue({ write: async () => {}, tidy: () => response.promise })
    queue.enqueue('one', 'Original', target, true)
    const tidy = queue.tidy(['one'])
    queue.discard(['one'])
    queue.enqueue('two', 'Later', target, true)
    response.resolve('Obsolete result')
    await tidy
    expect(get(queue).drafts.map(({ id, text }) => ({ id, text }))).toEqual([{ id: 'two', text: 'Later' }])
  })

  it('releases remaining speech unchanged if part of a tidy batch was discarded', async () => {
    const response = deferredText()
    const queue = createSpeechDraftQueue({ write: async () => {}, tidy: () => response.promise })
    queue.enqueue('one', 'First', target, true)
    queue.enqueue('two', 'Second', target, true)
    const tidy = queue.tidy(['one', 'two'])
    queue.discard(['one'])
    response.resolve('First and second')
    await tidy
    expect(get(queue).drafts).toEqual([expect.objectContaining({ id: 'two', text: 'Second', status: 'pending' })])
  })

  it('automatically tidies only new segments when confirmation and cleanup are both enabled', async () => {
    const inputs: string[] = []
    const writes: string[] = []
    const queue = createSpeechDraftQueue({
      write: async (_request, operation) => { if (operation.kind === 'appendSpeech') writes.push(operation.text) },
      tidy: async (text) => { inputs.push(text); return `${text}.` },
    })
    queue.enqueue('old', 'Old pending', target, true)
    const tidy = queue.enqueue('new', 'New pending', target, true, true)
    queue.enqueue('direct', 'Direct', target, false, true)
    await queue.settled()
    await tidy
    expect(inputs).toEqual(['New pending'])
    expect(writes).toEqual(['Direct'])
    expect(get(queue).drafts.map(({ text, cleanupState }) => ({ text, cleanupState }))).toEqual([
      { text: 'Old pending', cleanupState: 'pending' }, { text: 'New pending.', cleanupState: 'cleaned' },
    ])
  })

  it('preserves cleaned metadata through reload and marks edits as pending cleanup', async () => {
    const storage = memoryStorage()
    const first = createSpeechDraftQueue({ storage, write: async () => {}, tidy: async () => 'Cleaned' })
    first.enqueue('one', 'Original', target, true)
    await first.tidy(['one'])
    const restored = createSpeechDraftQueue({ storage, write: async () => {} })
    expect(groupSpeechDrafts(get(restored).drafts)[0].cleanupState).toBe('cleaned')
    restored.beginEdit(['one'])
    restored.saveEdit(['one'], 'Manually changed')
    expect(groupSpeechDrafts(get(restored).drafts)[0].cleanupState).toBe('pending')
  })

  it('never changes text previously attempted by the writer, including after reload', async () => {
    const storage = memoryStorage()
    const first = createSpeechDraftQueue({ storage, write: async () => { throw new Error('Acknowledgement lost') } })
    first.enqueue('one', 'Exact retry text', target, false)
    await first.settled()
    const writes: string[] = []
    const inputs: string[] = []
    const restored = createSpeechDraftQueue({ storage,
      write: async (_request, operation) => { if (operation.kind === 'appendSpeech') writes.push(operation.text) },
      tidy: async (text) => { inputs.push(text); return 'Changed' },
    })
    restored.beginEdit(['one'])
    await restored.tidy(['one'])
    expect(get(restored).edit).toBeNull()
    expect(inputs).toEqual([])
    expect(groupSpeechDrafts(get(restored).drafts)[0].editable).toBe(false)
    await restored.accept(['one'])
    expect(writes).toEqual(['Exact retry text'])
  })

  it('does not wait for network cleanup when settling recording writes', async () => {
    const response = deferredText()
    const queue = createSpeechDraftQueue({ write: async () => {}, tidy: () => response.promise })
    const tidy = queue.enqueue('one', 'Original', target, true, true)
    let settled = false
    void queue.settled().then(() => { settled = true })
    await Promise.resolve()
    expect(settled).toBe(true)
    response.resolve('Cleaned')
    await tidy
  })

  it('ignores old cleanup after disposal so a remounted queue keeps its current text', async () => {
    const response = deferredText()
    const storage = memoryStorage()
    const first = createSpeechDraftQueue({ storage, write: async () => {}, tidy: () => response.promise })
    first.enqueue('one', 'Original', target, true)
    const tidy = first.tidy(['one'])
    first.dispose()
    const second = createSpeechDraftQueue({ storage, write: async () => {} })
    second.beginEdit(['one'])
    second.saveEdit(['one'], 'Current text')
    response.resolve('Obsolete result')
    await tidy
    expect(get(createSpeechDraftQueue({ storage, write: async () => {} })).drafts[0].text).toBe('Current text')
  })
})

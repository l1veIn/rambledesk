import { get } from 'svelte/store'
import { describe, expect, it } from 'vitest'

import type { DraftOperation } from '../draftOperations'
import { handleSpeechDraftCommand } from './speechDraftCommands'
import { createSpeechDraftQueue, type SpeechTarget } from './speechDraftQueue'

const target: SpeechTarget = { requestId: 'request-a', requestTitle: 'Request A', action: null }
const otherTarget: SpeechTarget = { requestId: 'request-b', requestTitle: 'Request B', action: null }

function setup(tidy: (text: string) => Promise<string> = async (text) => `${text}.`) {
  const writes: Array<{ requestId: string; operation: DraftOperation }> = []
  const queue = createSpeechDraftQueue({
    write: async (requestId, operation) => { writes.push({ requestId, operation }) },
    tidy,
  })
  queue.enqueue('one', 'First words', target, true)
  return { queue, writes }
}

describe('speech draft command boundary', () => {
  it('leaves unrelated commands for the caller', async () => {
    const { queue, writes } = setup()
    const before = get(queue)
    for (const command of [null, undefined, 'accept-speech', [], {}, { type: 'toggle-recording' }]) {
      expect(await handleSpeechDraftCommand(queue, command)).toBe(false)
      expect(get(queue)).toBe(before)
    }
    expect(writes).toEqual([])
  })

  it('consumes malformed speech commands without mutating any draft', async () => {
    const { queue, writes } = setup()
    const before = get(queue)
    const types = ['accept-speech', 'discard-speech', 'tidy-speech', 'edit-speech', 'save-speech-edit', 'cancel-speech-edit']
    for (const type of types) {
      for (const ids of [undefined, null, 'one', [], [''], ['  '], ['one', 'one'], ['one', 2]]) {
        expect(await handleSpeechDraftCommand(queue, { type, ids, text: 'Changed' })).toBe(true)
        expect(get(queue)).toBe(before)
      }
    }
    expect(writes).toEqual([])
  })

  it('keeps the edit open for invalid text or stale save and cancel IDs', async () => {
    const { queue, writes } = setup()
    expect(await handleSpeechDraftCommand(queue, { type: 'edit-speech', ids: ['one'] })).toBe(true)
    expect(get(queue).edit).toEqual({ ids: ['one'], text: 'First words' })
    for (const text of [undefined, null, 3, '', ' \n ']) {
      await handleSpeechDraftCommand(queue, { type: 'save-speech-edit', ids: ['one'], text })
    }
    await handleSpeechDraftCommand(queue, { type: 'save-speech-edit', ids: ['missing'], text: 'Changed' })
    await handleSpeechDraftCommand(queue, { type: 'cancel-speech-edit', ids: ['missing'] })
    expect(get(queue).edit).toEqual({ ids: ['one'], text: 'First words' })
    expect(get(queue).drafts[0].text).toBe('First words')
    expect(writes).toEqual([])
  })

  it('blocks every review command while editing and preserves later arrivals after saving', async () => {
    const tidyInputs: string[] = []
    const { queue, writes } = setup(async (text) => { tidyInputs.push(text); return text })
    queue.enqueue('two', 'Second words', target, true)
    await handleSpeechDraftCommand(queue, { type: 'edit-speech', ids: ['one', 'two'] })
    expect(get(queue).edit?.ids).toEqual(['one', 'two'])
    queue.enqueue('late', 'Later arrival', otherTarget, true)
    for (const type of ['accept-speech', 'discard-speech', 'tidy-speech', 'edit-speech']) {
      await handleSpeechDraftCommand(queue, { type, ids: ['late'] })
    }
    await handleSpeechDraftCommand(queue, { type: 'accept-speech', ids: ['one', 'two', 'late'] })
    expect(writes).toEqual([])
    expect(tidyInputs).toEqual([])
    expect(get(queue).drafts.map(({ id }) => id)).toEqual(['one', 'two', 'late'])
    await handleSpeechDraftCommand(queue, { type: 'save-speech-edit', ids: ['one', 'two'], text: 'Edited words' })
    expect(get(queue).edit).toBeNull()
    await handleSpeechDraftCommand(queue, { type: 'accept-speech', ids: ['one'] })
    expect(writes).toEqual([{
      requestId: 'request-a',
      operation: expect.objectContaining({ kind: 'appendSpeech', segmentId: 'one', text: 'Edited words' }),
    }])
    expect(get(queue).drafts.map(({ id, text }) => ({ id, text }))).toEqual([{ id: 'late', text: 'Later arrival' }])
  })

  it('permits cancellation and discard under a lock while preventing changes and writes', async () => {
    const tidyInputs: string[] = []
    const { queue, writes } = setup(async (text) => { tidyInputs.push(text); return text })
    for (const type of ['accept-speech', 'tidy-speech', 'edit-speech']) {
      await handleSpeechDraftCommand(queue, { type, ids: ['one'] }, true)
    }
    expect(get(queue).edit).toBeNull()
    await handleSpeechDraftCommand(queue, { type: 'edit-speech', ids: ['one'] })
    await handleSpeechDraftCommand(queue, { type: 'save-speech-edit', ids: ['one'], text: 'Locked change' }, true)
    await handleSpeechDraftCommand(queue, { type: 'discard-speech', ids: ['one'] }, true)
    expect(get(queue).drafts[0].text).toBe('First words')
    expect(get(queue).edit?.ids).toEqual(['one'])
    await handleSpeechDraftCommand(queue, { type: 'cancel-speech-edit', ids: ['one'] }, true)
    expect(get(queue).edit).toBeNull()
    await handleSpeechDraftCommand(queue, { type: 'discard-speech', ids: ['one'] }, true)
    expect(get(queue).drafts).toEqual([])
    expect(writes).toEqual([])
    expect(tidyInputs).toEqual([])
  })

  it('tidies only the selected batch and confirms its result without consuming new speech', async () => {
    let finish!: (text: string) => void
    const response = new Promise<string>((resolve) => { finish = resolve })
    const { queue, writes } = setup(() => response)
    const tidy = handleSpeechDraftCommand(queue, { type: 'tidy-speech', ids: ['one'] })
    queue.enqueue('late', 'Keep this new speech', target, true)
    finish('First words, cleaned.')
    expect(await tidy).toBe(true)
    expect(get(queue).drafts[0]).toMatchObject({ id: 'one', text: 'First words, cleaned.', cleanupState: 'cleaned' })
    await handleSpeechDraftCommand(queue, { type: 'accept-speech', ids: ['one'] })
    expect(writes[0]).toEqual({ requestId: 'request-a', operation: {
      kind: 'appendSpeech', segmentId: 'one', text: 'First words, cleaned.', action: null, cleanupState: 'cleaned',
    } })
    expect(get(queue).drafts.map(({ id }) => id)).toEqual(['late'])
  })
})

// Adapted from Codeg composer/suggestion/mention-match tests at 3ebdfed.
// SPDX-License-Identifier: Apache-2.0
// RambleDesk: provider cancellation and session isolation are tested without invented resources.
import { Editor } from '@tiptap/core'
import { describe, expect, it, vi } from 'vitest'
import { buildComposerExtensions } from './editor-config'
import { textToSeededDoc } from './plain-text-content'
import { canStartMentionAt, createReferenceLookup, findReferenceQuery } from './reference-search'
import type { ComposerReference } from './types'

describe('provider-backed file mentions', () => {
  it.each(['看下@src', 'ユーザー@src', '𠀋@src', ' @src', '@src'])('recognizes a mention in %s', (text) => {
    const editor = new Editor({ element: null, extensions: buildComposerExtensions(), content: textToSeededDoc(text) })
    editor.commands.setTextSelection(editor.state.doc.content.size - 1)
    expect(findReferenceQuery(editor.state)?.text).toBe('src')
    editor.destroy()
  })
  it('does not offer file mentions inside email addresses', () => {
    expect(canStartMentionAt('me@example.com', 2)).toBe(false)
    expect(canStartMentionAt('a\u0323@example.com', 2)).toBe(false)
  })
  it('drops out-of-order and disposed searches even when the provider ignores AbortSignal', async () => {
    let finish!: (references: readonly ComposerReference[]) => void
    const provider = vi.fn().mockImplementationOnce(() => new Promise<readonly ComposerReference[]>((resolve) => { finish = resolve }))
      .mockResolvedValueOnce([{ id: 'two', label: 'Second', uri: 'file:///second' }, { id: 'fake', label: 'Unsupported', uri: 'codeg://embedded/fake' }])
    const lookup = createReferenceLookup()
    const first = lookup.search(provider, 'first')
    const second = await lookup.search(provider, 'second')
    expect(provider.mock.calls[0][1].signal.aborted).toBe(true)
    finish([{ id: 'one', label: 'First', uri: 'file:///first' }])
    expect(await first).toBeNull()
    expect(second).toEqual([{ id: 'two', label: 'Second', uri: 'file:///second' }])
    provider.mockImplementationOnce(() => new Promise<readonly ComposerReference[]>((resolve) => { finish = resolve }))
    const disposed = lookup.search(provider, 'third')
    lookup.cancel()
    finish([])
    expect(await disposed).toBeNull()
  })
})

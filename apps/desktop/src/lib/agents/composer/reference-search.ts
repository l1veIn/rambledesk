// Adapted from Codeg composer/suggestion/mention-match.ts at 3ebdfed.
// SPDX-License-Identifier: Apache-2.0
// RambleDesk: direct ProseMirror matching; no @tiptap/suggestion or React dependency.
import type { EditorState } from '@tiptap/pm/state'
import type { ComposerReference, ReferenceSearch } from './types'

const ALLOWED_MENTION_PREFIX = /^[\s\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}\p{Script=Hangul}\p{Script=Bopomofo}　-〿︰-﹏＀-￯·゠・ー]$/u

export function canStartMentionAt(text: string, index: number): boolean {
  if (index <= 0) return true
  const unit = text.charCodeAt(index - 1)
  const prefix = unit >= 0xdc00 && unit <= 0xdfff && index >= 2 ? text.slice(index - 2, index) : text.charAt(index - 1)
  return ALLOWED_MENTION_PREFIX.test(prefix)
}

export type ReferenceQuery = Readonly<{ from: number; to: number; text: string }>
export function findReferenceQuery(state: EditorState): ReferenceQuery | null {
  if (!state.selection.empty) return null
  const before = state.selection.$from.nodeBefore
  if (!before?.isText) return null
  const text = before.text ?? ''
  const index = text.lastIndexOf('@')
  if (index < 0 || !canStartMentionAt(text, index)) return null
  const query = text.slice(index + 1)
  if (/[\s@]/u.test(query)) return null
  return { from: state.selection.from - text.length + index, to: state.selection.from, text: query }
}

/** Providers may ignore AbortSignal; the generation still prevents late results leaking across drafts. */
export function createReferenceLookup() {
  let generation = 0
  let controller: AbortController | null = null
  function cancel() { generation += 1; controller?.abort(); controller = null }
  async function search(provider: ReferenceSearch, query: string): Promise<readonly ComposerReference[] | null> {
    cancel()
    const mine = generation
    controller = new AbortController()
    try {
      const results = await provider(query, { signal: controller.signal })
      return mine === generation ? results.filter((item) => item.uri.toLowerCase().startsWith('file:')) : null
    } catch (error) {
      if (mine !== generation) return null
      throw error
    }
  }
  return { search, cancel }
}

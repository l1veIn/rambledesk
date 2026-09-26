import type { Editor } from '@tiptap/core'
import { fieldDocumentText, fieldTextDocument } from './fieldTextDocument'

/** Patch only the changed span, so background speech keeps the caret and undo history. */
export function syncFieldText(editor: Editor, text: string): void {
  if (fieldDocumentText(editor.getJSON()) === text) return
  const next = editor.schema.nodeFromJSON(fieldTextDocument(text))
  const current = editor.state.doc
  const start = current.content.findDiffStart(next.content)
  if (start === null) return
  const end = current.content.findDiffEnd(next.content)!
  const overlap = start - Math.min(end.a, end.b)
  const fromEnd = overlap > 0 ? end.a + overlap : end.a
  const toEnd = overlap > 0 ? end.b + overlap : end.b
  editor.view.dispatch(editor.state.tr.replace(start, fromEnd, next.slice(start, toEnd))
    .setMeta('addToHistory', false).setMeta('preventUpdate', true))
}

function changedSpan(before: string[], after: string[]) {
  let start = 0
  while (start < before.length && start < after.length && before[start] === after[start]) start++
  let end = before.length, nextEnd = after.length
  while (end > start && nextEnd > start && before[end - 1] === after[nextEnd - 1]) { end--; nextEnd-- }
  return { start, end, inserted: after.slice(start, nextEnd) }
}

/** A composing editor may receive a speech append before the browser commits its
 * local edit. Rebase disjoint changes; direct manual edits win overlapping text. */
export function rebaseFieldTextEdit(before: string, edited: string, latest: string): string {
  if (latest === before || latest === edited) return edited
  if (before === edited) return latest
  const source = [...before], local = changedSpan(source, [...edited])
  const current = [...latest], remote = changedSpan(source, current)
  const delta = remote.inserted.length - (remote.end - remote.start)
  if (local.end <= remote.start) {
    current.splice(local.start, local.end - local.start, ...local.inserted)
    return current.join('')
  }
  if (local.start >= remote.end) {
    current.splice(local.start + delta, local.end - local.start, ...local.inserted)
    return current.join('')
  }
  // An overlapping replacement cannot be merged without guessing the wording.
  // Keep the user's current text; disjoint speech appends took the paths above.
  return edited
}

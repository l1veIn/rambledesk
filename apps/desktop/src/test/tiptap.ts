import type { Editor } from '@tiptap/core'

/** TipTap exposes its mounted Editor on the contenteditable DOM element. */
export function tiptapEditor(element: HTMLElement | null): Editor {
  const editor = (element as (HTMLElement & { editor?: Editor }) | null)?.editor
  if (!editor) throw new Error('Expected a mounted TipTap input')
  return editor
}

/** Exercise ProseMirror's ordinary transaction/update path, including undo. */
export function replaceInputText(element: HTMLElement | null, text: string): void {
  const editor = tiptapEditor(element)
  const paragraph = editor.schema.nodes.paragraph.create(null, text ? editor.schema.text(text) : null)
  editor.view.dispatch(editor.state.tr.replaceWith(0, editor.state.doc.content.size, paragraph))
}

export function inputText(element: HTMLElement | null): string {
  return tiptapEditor(element).getText({ blockSeparator: '\n' })
}

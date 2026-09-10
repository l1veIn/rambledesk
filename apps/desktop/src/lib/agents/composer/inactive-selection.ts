// Adapted from xintaofei/codeg, commit 3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1.
// SPDX-License-Identifier: Apache-2.0
// RambleDesk: local imports, framework-free extensions, and scoped CSS names.
import { Extension } from "@tiptap/core"
import { Plugin, PluginKey, type EditorState } from "@tiptap/pm/state"
import { Decoration, DecorationSet } from "@tiptap/pm/view"

/** CSS class painted over the selected range while the editor is unfocused. */
export const INACTIVE_SELECTION_CLASS = "ramble-composer-inactive-selection"

/** Plugin state is a single boolean: whether the editor currently has focus. */
export const inactiveSelectionKey = new PluginKey<boolean>(
  "inactiveSelectionHighlight"
)

/**
 * Decorations that paint the current selection when the editor is NOT focused.
 *
 * Pure so it can be unit-tested against a real editor state. Returns `null` when
 * the editor is focused (the browser paints the native selection) or when the
 * selection is empty (nothing to highlight).
 */
export function inactiveSelectionDecorations(
  state: EditorState,
  focused: boolean
): DecorationSet | null {
  if (focused) return null
  const { from, to } = state.selection
  if (from >= to) return null
  return DecorationSet.create(state.doc, [
    Decoration.inline(from, to, { class: INACTIVE_SELECTION_CLASS }),
  ])
}

/**
 * Keep the text selection visible after the editor loses focus.
 *
 * When focus moves to an overlay that preserves the selection — most visibly the
 * composer's custom right-click menu, whose radix content takes focus — browsers
 * stop painting the active selection: Chromium/Firefox dim it to a faint
 * "inactive" colour, and WebKit (the macOS desktop webview) drops it entirely,
 * so a CSS `::selection` rule has nothing left to restyle. This extension paints
 * the selection itself instead: while the editor is blurred it decorates the
 * current `state.selection` (which survives blur — it only changes via
 * transactions) with {@link INACTIVE_SELECTION_CLASS}, a normal element
 * background that renders regardless of focus or platform.
 *
 * The decoration exists only while blurred, so a focused editor keeps its native
 * selection untouched and editing / IME are never affected. Focus is tracked in
 * plugin state, toggled by a meta-only transaction (no doc change → no
 * `onUpdate` / draft save) from the editor's own focus/blur events.
 */
export const InactiveSelectionHighlight = Extension.create({
  name: "inactiveSelectionHighlight",

  addProseMirrorPlugins() {
    return [
      new Plugin<boolean>({
        key: inactiveSelectionKey,
        state: {
          // Start unfocused; the focus listener corrects this the moment the
          // editor gains focus, and an empty initial selection paints nothing.
          init: () => false,
          apply: (tr, focused) => {
            const meta = tr.getMeta(inactiveSelectionKey)
            return typeof meta === "boolean" ? meta : focused
          },
        },
        props: {
          decorations(state) {
            return inactiveSelectionDecorations(
              state,
              inactiveSelectionKey.getState(state) ?? false
            )
          },
        },
        view(view) {
          const sync = (focused: boolean) => {
            // Guard against redundant dispatches (and any focus/blur loop).
            if (inactiveSelectionKey.getState(view.state) === focused) return
            view.dispatch(view.state.tr.setMeta(inactiveSelectionKey, focused))
          }
          const onFocus = () => sync(true)
          const onBlur = () => sync(false)
          view.dom.addEventListener("focus", onFocus)
          view.dom.addEventListener("blur", onBlur)
          return {
            destroy() {
              view.dom.removeEventListener("focus", onFocus)
              view.dom.removeEventListener("blur", onBlur)
            },
          }
        },
      }),
    ]
  },
})

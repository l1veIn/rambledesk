// Adapted from xintaofei/codeg, commit 3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1.
// SPDX-License-Identifier: Apache-2.0
// RambleDesk: local imports, framework-free extensions, and scoped CSS names.
/**
 * Pure keyboard-decision logic for the composer, extracted so the IME / code
 * block / list precedence can be unit-tested exhaustively without driving a
 * real ProseMirror view (jsdom can't emulate IME composition reliably).
 */

import { isImeCompositionKey } from "./ime-composition"
import { matchShortcutEvent } from "./keyboard-shortcuts"

/** The subset of a keydown event the submit decision depends on. */
export interface SubmitKeyEvent {
  key: string
  shiftKey: boolean
  altKey: boolean
  ctrlKey: boolean
  metaKey: boolean
  /** Standard DOM flag: a composition (IME) is in flight. */
  isComposing: boolean
  /** Legacy 229 sentinel some IMEs report on the composition-confirming key. */
  keyCode: number
}

/** Editor context that overrides plain Enter-to-submit with structural Enter. */
export interface SubmitKeyContext {
  /** ProseMirror `view.composing` — composition in flight (second IME signal). */
  composing: boolean
  /** Caret is inside a code block → Enter inserts a newline. */
  inCodeBlock: boolean
  /** Caret is inside a list item → Enter creates/exits the list item. */
  inList: boolean
}

/**
 * Decide whether a keydown should trigger submit. Returns `true` only for a
 * plain Enter (no modifiers), while not composing, and not inside a code block
 * or list. In every other case the editor keeps its default behavior (newline /
 * list split / IME confirm).
 */
export function shouldSubmitOnEnter(
  event: SubmitKeyEvent,
  context: SubmitKeyContext
): boolean {
  if (event.key !== "Enter") return false
  if (event.shiftKey || event.altKey || event.ctrlKey || event.metaKey) {
    return false
  }
  // IME guard: never submit while a composition is in flight. The Enter that
  // confirms a CJK candidate reports isComposing / keyCode 229 / view.composing.
  if (isImeCompositionKey(event) || context.composing) return false
  // Structural Enter inside code blocks and lists (per the composer design).
  if (context.inCodeBlock || context.inList) return false
  return true
}

/** Configurable submit / newline key bindings (matchShortcutEvent strings). */
export interface ComposerKeyBindings {
  /** Binding that sends the message. Default `"enter"`. */
  submit: string
  /** Binding that inserts a line break instead of sending. Default `"shift+enter"`. */
  newline: string
}

/** What a keydown should do in the composer, or `null` to keep the editor default. */
export type ComposerKeyAction = "submit" | "newline" | null

/**
 * Generalizes {@link shouldSubmitOnEnter} to the user-configurable submit /
 * newline bindings (`send_message` / `newline_in_message`). Pure, so the
 * precedence is unit-testable without a live view.
 *
 * Precedence:
 * 1. Never act mid-IME-composition (the CJK candidate-confirming Enter).
 * 2. A *bare* Enter inside a code block or list keeps ProseMirror's structural
 *    default (newline / list split) — it is never hijacked into submit or a
 *    forced break, regardless of the bindings.
 * 3. The submit binding wins over the newline binding when both match.
 *
 * The newline binding is resolved explicitly (rather than deferring to the
 * editor keymap) because bindings are free-form and may not correspond to a key
 * ProseMirror binds.
 */
export function decideComposerKey(
  event: SubmitKeyEvent,
  context: SubmitKeyContext,
  bindings: ComposerKeyBindings
): ComposerKeyAction {
  if (isImeCompositionKey(event) || context.composing) return null
  const bareEnter =
    event.key === "Enter" &&
    !event.shiftKey &&
    !event.altKey &&
    !event.ctrlKey &&
    !event.metaKey
  if (bareEnter && (context.inCodeBlock || context.inList)) return null

  if (matchShortcutEvent(event, bindings.submit)) return "submit"
  if (matchShortcutEvent(event, bindings.newline)) return "newline"
  return null
}

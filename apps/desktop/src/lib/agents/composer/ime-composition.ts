// Adapted from xintaofei/codeg, commit 3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1.
// SPDX-License-Identifier: Apache-2.0
// Upstream src/lib/ime-composition.ts; pure IME guard retained.
/**
 * Cross-browser detection of the keydown that belongs to an IME composition.
 *
 * A CJK user picks a candidate with Enter. That Enter must never reach an
 * "Enter submits" handler, or typing `nihao` + Enter commits / renames /
 * searches instead of inserting 你好. No single flag catches every engine:
 *
 * - `isComposing` — the standard flag, set while a composition is in flight.
 *   Chromium and Gecko report it on the candidate-confirming key.
 * - `keyCode === 229` — the legacy sentinel. WebKit (Safari, and the WKWebView
 *   the desktop app renders in) fires `compositionend` *before* the confirming
 *   `keydown`, so `isComposing` is already false by then and 229 is the only
 *   trace the event still carries. This is the signal the plain form controls
 *   were missing.
 * - `key === "Process"` — what Gecko reports for a key the IME swallowed.
 * - A flag tracked from `compositionstart` / `compositionend` — the fallback
 *   for engines that report none of the above but do bracket the composition
 *   with the DOM events. Lives in `useImeGuard`, which composes with this.
 *
 * The rich composer already runs the same three in-event signals plus
 * ProseMirror's `view.composing` (see `chat/composer/submit-key.ts`); this
 * module is that guard for every plain input and textarea around the app.
 */

/** Legacy `keyCode` an IME reports for the key it is processing. */
export const IME_PROCESS_KEY_CODE = 229

/**
 * The parts of a keydown the composition check reads. Shaped to accept both a
 * native `KeyboardEvent` and a React synthetic one — React exposes `keyCode`
 * on the synthetic event but keeps `isComposing` on `nativeEvent`, so both
 * levels are read and neither is required.
 */
export interface ImeKeyEvent {
  key: string
  keyCode?: number
  isComposing?: boolean
  nativeEvent?: { isComposing?: boolean; keyCode?: number }
}

/** True when this keydown is the IME's own, not a command meant for the app. */
export function isImeCompositionKey(event: ImeKeyEvent): boolean {
  const native = event.nativeEvent
  return (
    event.isComposing === true ||
    native?.isComposing === true ||
    event.keyCode === IME_PROCESS_KEY_CODE ||
    native?.keyCode === IME_PROCESS_KEY_CODE ||
    event.key === "Process"
  )
}

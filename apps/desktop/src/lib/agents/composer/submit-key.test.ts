// Adapted from xintaofei/codeg, commit 3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1.
// SPDX-License-Identifier: Apache-2.0
// RambleDesk: local imports, framework-free extensions, and scoped CSS names.
import { describe, expect, it } from "vitest"

import {
  decideComposerKey,
  shouldSubmitOnEnter,
  type ComposerKeyBindings,
  type SubmitKeyContext,
  type SubmitKeyEvent,
} from "./submit-key"

const plainEnter: SubmitKeyEvent = {
  key: "Enter",
  shiftKey: false,
  altKey: false,
  ctrlKey: false,
  metaKey: false,
  isComposing: false,
  keyCode: 13,
}

const topLevel: SubmitKeyContext = {
  composing: false,
  inCodeBlock: false,
  inList: false,
}

describe("shouldSubmitOnEnter", () => {
  it("submits on a plain Enter at the top level", () => {
    expect(shouldSubmitOnEnter(plainEnter, topLevel)).toBe(true)
  })

  it("ignores non-Enter keys", () => {
    expect(shouldSubmitOnEnter({ ...plainEnter, key: "a" }, topLevel)).toBe(
      false
    )
  })

  it.each([
    ["Shift", { shiftKey: true }],
    ["Alt", { altKey: true }],
    ["Ctrl", { ctrlKey: true }],
    ["Meta", { metaKey: true }],
  ])("does not submit with the %s modifier (newline / shortcut)", (_n, mod) => {
    expect(shouldSubmitOnEnter({ ...plainEnter, ...mod }, topLevel)).toBe(false)
  })

  describe("IME guard", () => {
    it("does not submit while event.isComposing", () => {
      expect(
        shouldSubmitOnEnter({ ...plainEnter, isComposing: true }, topLevel)
      ).toBe(false)
    })

    it("does not submit on the legacy keyCode 229 sentinel", () => {
      expect(
        shouldSubmitOnEnter({ ...plainEnter, keyCode: 229 }, topLevel)
      ).toBe(false)
    })

    it("does not submit while view.composing", () => {
      expect(
        shouldSubmitOnEnter(plainEnter, { ...topLevel, composing: true })
      ).toBe(false)
    })
  })

  describe("structural Enter", () => {
    it("does not submit inside a code block", () => {
      expect(
        shouldSubmitOnEnter(plainEnter, { ...topLevel, inCodeBlock: true })
      ).toBe(false)
    })

    it("does not submit inside a list item", () => {
      expect(
        shouldSubmitOnEnter(plainEnter, { ...topLevel, inList: true })
      ).toBe(false)
    })
  })

  it("submits on Enter immediately after composition ends (no IME flags set)", () => {
    // Post-composition Enter: isComposing false, keyCode normal, view not
    // composing — this is a genuine submit, not a candidate confirmation.
    expect(shouldSubmitOnEnter(plainEnter, topLevel)).toBe(true)
  })
})

describe("decideComposerKey", () => {
  const DEFAULT: ComposerKeyBindings = {
    submit: "enter",
    newline: "shift+enter",
  }
  const SWAPPED: ComposerKeyBindings = { submit: "mod+enter", newline: "enter" }

  describe("default bindings (enter / shift+enter)", () => {
    it("submits on a plain Enter at the top level", () => {
      expect(decideComposerKey(plainEnter, topLevel, DEFAULT)).toBe("submit")
    })

    it("inserts a newline on Shift+Enter", () => {
      expect(
        decideComposerKey({ ...plainEnter, shiftKey: true }, topLevel, DEFAULT)
      ).toBe("newline")
    })

    it("keeps the editor default for a bare Enter in a code block", () => {
      expect(
        decideComposerKey(
          plainEnter,
          { ...topLevel, inCodeBlock: true },
          DEFAULT
        )
      ).toBeNull()
    })

    it("keeps the editor default for a bare Enter in a list", () => {
      expect(
        decideComposerKey(plainEnter, { ...topLevel, inList: true }, DEFAULT)
      ).toBeNull()
    })

    it("does nothing for an unbound modified Enter (mod+enter)", () => {
      expect(
        decideComposerKey({ ...plainEnter, metaKey: true }, topLevel, DEFAULT)
      ).toBeNull()
    })

    it.each([
      ["isComposing", { ...plainEnter, isComposing: true }, topLevel],
      ["keyCode 229", { ...plainEnter, keyCode: 229 }, topLevel],
      ["view.composing", plainEnter, { ...topLevel, composing: true }],
    ] as const)("never acts mid-composition (%s)", (_n, event, context) => {
      expect(decideComposerKey(event, context, DEFAULT)).toBeNull()
    })
  })

  describe("swapped bindings (mod+enter submits, enter = newline)", () => {
    it.each([
      ["meta", { metaKey: true }],
      ["ctrl", { ctrlKey: true }],
    ])("submits on %s+Enter", (_n, mod) => {
      expect(
        decideComposerKey({ ...plainEnter, ...mod }, topLevel, SWAPPED)
      ).toBe("submit")
    })

    it("treats a plain Enter as a newline", () => {
      expect(decideComposerKey(plainEnter, topLevel, SWAPPED)).toBe("newline")
    })

    it("still keeps the structural default for a bare Enter in a list", () => {
      expect(
        decideComposerKey(plainEnter, { ...topLevel, inList: true }, SWAPPED)
      ).toBeNull()
    })

    it("submits on Mod+Enter even inside a code block (not a bare Enter)", () => {
      expect(
        decideComposerKey(
          { ...plainEnter, metaKey: true },
          { ...topLevel, inCodeBlock: true },
          SWAPPED
        )
      ).toBe("submit")
    })
  })

  it("prefers submit over newline when both bindings match", () => {
    expect(
      decideComposerKey(plainEnter, topLevel, {
        submit: "enter",
        newline: "enter",
      })
    ).toBe("submit")
  })
})

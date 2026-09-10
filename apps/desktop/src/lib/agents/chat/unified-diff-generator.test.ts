// Adapted from xintaofei/codeg, commit 3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1.
// SPDX-License-Identifier: Apache-2.0
// RambleDesk: pure diff logic retained; imports point to the local extracted engine.
import { describe, expect, it } from "vitest"

import {
  countUnifiedDiffLineChanges,
  estimateChangedLineStats,
} from "./line-change-stats"
import { generateUnifiedDiff } from "./unified-diff-generator"

function makeLines(count: number, label = "line"): string[] {
  return Array.from({ length: count }, (_, i) => `${label} ${i + 1}`)
}

/** Count diff body lines (excludes the `--- a/` / `+++ b/` file header). */
function bodyLineCount(diff: string): number {
  return diff.split("\n").filter((l) => !/^(---|\+\+\+) /.test(l)).length
}

describe("generateUnifiedDiff", () => {
  it("returns null when there is no change", () => {
    expect(generateUnifiedDiff("a\nb\nc", "a\nb\nc")).toBeNull()
    expect(generateUnifiedDiff("", "")).toBeNull()
    // Differs only by a trailing newline → splitLines normalizes both away.
    expect(generateUnifiedDiff("a\nb\n", "a\nb")).toBeNull()
  })

  // The core regression: a one-line edit inside a large file used to trip the
  // crude `oldLines.length * newLines.length > 200_000` gate and dump the whole
  // file as -/+ lines (Codex ACP edits arrive as full-file old/new strings).
  it("keeps a one-line change in a 1000-line file compact", () => {
    const oldLines = makeLines(1000)
    const newLines = [...oldLines]
    newLines[499] = "CHANGED line 500"

    const diff = generateUnifiedDiff(
      oldLines.join("\n"),
      newLines.join("\n"),
      "big.ts"
    )

    expect(diff).not.toBeNull()
    const text = diff as string

    // Bounded: a single hunk with ~3 lines of context, not ~2000 dumped lines.
    expect(bodyLineCount(text)).toBeLessThan(20)
    // Real file line numbers (change at line 500, 3 lines of leading context).
    expect(text).toContain("@@ -497,7 +497,7 @@")
    expect(text).toContain("-line 500")
    expect(text).toContain("+CHANGED line 500")
    // The untouched bulk of the file is never emitted.
    expect(text).not.toContain("line 800")
    expect(text).not.toContain("line 200")
  })

  it("emits separate compact hunks for scattered changes, skipping the gap", () => {
    const oldLines = makeLines(1000)
    const newLines = [...oldLines]
    newLines[1] = "CHANGED line 2"
    newLines[898] = "CHANGED line 899"

    const diff = generateUnifiedDiff(
      oldLines.join("\n"),
      newLines.join("\n"),
      "big.ts"
    )
    expect(diff).not.toBeNull()
    const text = diff as string

    // Two distinct hunks, not one giant span covering the whole file.
    expect((text.match(/^@@ /gm) ?? []).length).toBe(2)
    expect(bodyLineCount(text)).toBeLessThan(40)
    // The hundreds of unchanged lines between the two edits stay out.
    expect(text).not.toContain("line 500")
    expect(text).toContain("+CHANGED line 2")
    expect(text).toContain("+CHANGED line 899")
  })

  it("renders a pure insertion with no deletion lines and correct anchoring", () => {
    const oldLines = makeLines(10)
    const newLines = [
      ...oldLines.slice(0, 5),
      "inserted line",
      ...oldLines.slice(5),
    ]

    const diff = generateUnifiedDiff(
      oldLines.join("\n"),
      newLines.join("\n"),
      "f.ts"
    )
    expect(diff).not.toBeNull()
    const text = diff as string

    const stats = countUnifiedDiffLineChanges(text)
    expect(stats).toEqual({ additions: 1, deletions: 0 })
    expect(text).toContain("+inserted line")
    // Context anchored around the insertion point, not from line 1.
    expect(text).toContain("@@ -3,6 +3,7 @@")
  })

  it("renders a pure deletion with no addition lines", () => {
    const oldLines = makeLines(10)
    const newLines = oldLines.filter((_, i) => i !== 4)

    const diff = generateUnifiedDiff(
      oldLines.join("\n"),
      newLines.join("\n"),
      "f.ts"
    )
    expect(diff).not.toBeNull()
    const text = diff as string

    const stats = countUnifiedDiffLineChanges(text)
    expect(stats).toEqual({ additions: 0, deletions: 1 })
    expect(text).toContain("-line 5")
  })

  // A later hunk's `+start` must include the net line delta of earlier hunks.
  // An insertion near the top shifts every following line down by one in the new
  // file, so the second hunk's new-side start is old start + 1.
  it("offsets a later hunk's +start by earlier insertions", () => {
    const oldLines = makeLines(200)
    const newLines = [...oldLines]
    newLines[149] = "CHANGED line 150" // replacement near the bottom
    newLines.splice(2, 0, "INSERTED") // insertion near the top (net +1)

    const diff = generateUnifiedDiff(
      oldLines.join("\n"),
      newLines.join("\n"),
      "f.ts"
    )
    expect(diff).not.toBeNull()
    const text = diff as string

    expect((text.match(/^@@ /gm) ?? []).length).toBe(2)
    // First hunk: the insertion (old start 1, new gains a line).
    expect(text).toContain("@@ -1,5 +1,6 @@")
    // Second hunk: old start 147 but new start 148 — shifted by the insertion.
    expect(text).toContain("@@ -147,7 +148,7 @@")
  })

  // A genuine full rewrite (no common prefix/suffix) still exceeds the budget
  // and falls back to the naive diff — that path is intentional and correct;
  // the fix only stops localized edits from reaching it.
  it("falls back to a complete naive diff for a true large rewrite", () => {
    // 2500 disjoint lines on each side → no common prefix/suffix to trim and a
    // 2500×2500 window that exceeds the shared LINE_DIFF_LCS_BUDGET, so the
    // naive path runs (and the collapsed stat falls back in lockstep).
    const oldText = makeLines(2500, "old").join("\n")
    const newText = makeLines(2500, "new").join("\n")

    const diff = generateUnifiedDiff(oldText, newText, "rewrite.ts")
    expect(diff).not.toBeNull()
    const text = diff as string

    expect(text).toContain("@@ -1,2500 +1,2500 @@")
    expect(countUnifiedDiffLineChanges(text)).toEqual({
      additions: 2500,
      deletions: 2500,
    })
  })

  // countUnifiedDiffLineChanges must not mistake body content for a file header.
  it("counts body lines whose content starts with +++/---", () => {
    const oldText = ["keep", "--- old marker", "keep2"].join("\n")
    const newText = ["keep", "+++ new marker", "keep2"].join("\n")

    const diff = generateUnifiedDiff(oldText, newText, "f.ts")
    expect(diff).not.toBeNull()
    const text = diff as string

    // Marker + content: "-" + "--- old marker" = "---- old marker", etc.
    expect(text).toContain("---- old marker")
    expect(text).toContain("++++ new marker")
    expect(countUnifiedDiffLineChanges(text)).toEqual({
      additions: 1,
      deletions: 1,
    })
  })

  // Guards the core invariant the user asked for: the collapsed header stat
  // (estimateChangedLineStats) and the expanded diff (generateUnifiedDiff) must
  // report the SAME +/- counts. Includes the divergence cases reviewers found:
  // a large duplicate prefix, a large sparse window, and duplicate lines INSIDE
  // the changed window — all previously tripped one path's fallback but not the
  // other's. They now share one trim + budget, so they agree by construction.
  it("agrees with estimateChangedLineStats on +/- counts", () => {
    const bigOld = makeLines(1000)
    const bigNew = [...bigOld]
    bigNew[499] = "CHANGED line 500"

    // Insertion near the top + replacement near the bottom (net line offset).
    const offOld = makeLines(200)
    const offNew = [...offOld]
    offNew[149] = "CHANGED line 150"
    offNew.splice(2, 0, "INSERTED")

    // Large duplicate prefix the diff trims but the stat's pair budget used to
    // trip on (500*500 matching "x" pairs > 200_000).
    const dupPrefix = Array.from({ length: 500 }, () => "x")
    const dupOld = [...dupPrefix, "old one", "shared middle", "old two"]
    const dupNew = [...dupPrefix, "new one", "shared middle", "new two"]

    // Duplicate lines INSIDE the window (no common prefix/suffix to trim). The
    // stat's old pair budget tripped (500*500 "same" pairs) → +502, while the
    // diff DP'd the 502-line window → +2. Now both stay under one budget → +2.
    const sameBlock = Array.from({ length: 500 }, () => "same")
    const dupInnerOld = ["old-start", ...sameBlock, "old-end"].join("\n")
    const dupInnerNew = ["new-start", ...sameBlock, "new-end"].join("\n")

    // Large sparse window: 1001-line changed region with one interior match.
    const sparseOld = [
      ...makeLines(500, "old-before"),
      "shared",
      ...makeLines(500, "old-after"),
    ].join("\n")
    const sparseNew = [
      ...makeLines(500, "new-before"),
      "shared",
      ...makeLines(500, "new-after"),
    ].join("\n")

    // Body lines whose content itself begins with +++/--- (emitted as ++++/----)
    // must be counted, not mistaken for the file header — exercises both the
    // counter and the two paths agreeing on marker-like content.
    const markerOld = ["keep", "--- old marker", "keep2"].join("\n")
    const markerNew = ["keep", "+++ new marker", "keep2"].join("\n")

    const cases: Array<[string, string]> = [
      ["a\nb\nc\nd", "a\nB\nc\nd"],
      ["a\nb\nc", "a\nx\ny\nc"],
      ["one\ntwo\nthree\nfour\nfive", "one\nthree\nfour"],
      [bigOld.join("\n"), bigNew.join("\n")],
      [offOld.join("\n"), offNew.join("\n")],
      [dupOld.join("\n"), dupNew.join("\n")],
      [dupInnerOld, dupInnerNew],
      [sparseOld, sparseNew],
      [markerOld, markerNew],
      // Reviewer's exact counterexample: a lone line whose content is `-- …` /
      // `++ …`, emitted as `--- …` / `+++ …` (collides with the header prefix).
      ["-- old marker", "++ new marker"],
    ]

    for (const [oldText, newText] of cases) {
      const diff = generateUnifiedDiff(oldText, newText)
      expect(diff).not.toBeNull()
      expect(countUnifiedDiffLineChanges(diff as string)).toEqual(
        estimateChangedLineStats(oldText, newText)
      )
    }
  })
})

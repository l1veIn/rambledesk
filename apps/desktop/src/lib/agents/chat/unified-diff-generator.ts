// Adapted from xintaofei/codeg, commit 3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1.
// SPDX-License-Identifier: Apache-2.0
// RambleDesk: pure diff logic retained; imports point to the local extracted engine.
import { computeLineDiff, type DiffHunk } from "./line-diff"
import { exceedsLineDiffBudget } from "./line-change-stats"

/**
 * Generate a unified diff string from old and new text.
 *
 * Uses LCS-based line diff when within budget, falls back to
 * simple "all deletions then all additions" for very large inputs.
 */
export function generateUnifiedDiff(
  oldText: string,
  newText: string,
  filePath?: string,
  contextLines: number = 3
): string | null {
  if (!oldText && !newText) return null
  if (oldText === newText) return null

  const oldLines = oldText ? splitLines(oldText) : []
  const newLines = newText ? splitLines(newText) : []

  const path = filePath ?? "file"
  const header = `--- a/${path}\n+++ b/${path}`

  // Diff only the changed window: trim the common leading/trailing lines so the
  // O(n*m) LCS — and the naive fallback below — operate on the actual change
  // region instead of the whole file. This mirrors `contiguousChangedLineStats`
  // in line-change-stats.ts, keeping this expanded diff in sync with the
  // collapsed +N/-M stat (which already trims this way). Without it, a small
  // edit inside a large file (e.g. the full-file old/new strings synthesized
  // for a Codex ACP edit) trips the budget gate and dumps the entire file.
  let prefix = 0
  while (
    prefix < oldLines.length &&
    prefix < newLines.length &&
    oldLines[prefix] === newLines[prefix]
  ) {
    prefix += 1
  }

  let suffix = 0
  while (
    suffix < oldLines.length - prefix &&
    suffix < newLines.length - prefix &&
    oldLines[oldLines.length - 1 - suffix] ===
      newLines[newLines.length - 1 - suffix]
  ) {
    suffix += 1
  }

  const midOld = oldLines.slice(prefix, oldLines.length - suffix)
  const midNew = newLines.slice(prefix, newLines.length - suffix)

  // Nothing left after trimming (content differed only by a trailing newline).
  if (midOld.length === 0 && midNew.length === 0) return null

  // Fall back to the naive diff when the changed window is too large for an
  // exact LCS. This gate is shared with the collapsed +N/−M stat
  // (line-change-stats.ts) so the two always agree: below the budget both
  // compute the exact LCS; at/above it both treat the whole window as changed.
  // The window is already trimmed, so the naive output covers only the real
  // change region, never the whole file.
  if (exceedsLineDiffBudget(midOld, midNew)) {
    return buildNaiveDiff(header, midOld, midNew, prefix)
  }

  const hunks = computeLineDiff(midOld, midNew)
  if (hunks.length === 0) return null

  // Shift hunk positions from window-relative back to full-file coordinates so
  // buildUnifiedHunks can pull surrounding context from the full old array.
  const offsetHunks =
    prefix === 0
      ? hunks
      : hunks.map((hunk) => ({ ...hunk, baseStart: hunk.baseStart + prefix }))

  const unifiedHunks = buildUnifiedHunks(oldLines, offsetHunks, contextLines)

  return `${header}\n${unifiedHunks}`
}

function splitLines(text: string): string[] {
  const lines = text.split("\n")
  // Remove trailing empty line from trailing newline
  if (lines.length > 0 && lines[lines.length - 1] === "") {
    lines.pop()
  }
  return lines
}

/**
 * Naive diff: all deletions first, then all additions, with a single hunk
 * header. Used as a fallback when the changed window is too large for LCS.
 *
 * `windowStart` is the 0-based index of the window within the full file (the
 * common-prefix length), so the hunk header reports real file line numbers.
 */
function buildNaiveDiff(
  header: string,
  oldLines: string[],
  newLines: string[],
  windowStart: number
): string {
  const oldStart = oldLines.length === 0 ? 0 : windowStart + 1
  const newStart = newLines.length === 0 ? 0 : windowStart + 1
  const hunkHeader = `@@ -${oldStart},${oldLines.length} +${newStart},${newLines.length} @@`

  const parts = [header, hunkHeader]
  for (const line of oldLines) parts.push(`-${line}`)
  for (const line of newLines) parts.push(`+${line}`)
  return parts.join("\n")
}

/**
 * Convert DiffHunk[] into unified diff text with context lines and hunk headers.
 *
 * Groups nearby hunks that overlap in their context windows into a single
 * unified hunk, producing output similar to `diff -u`.
 */
function buildUnifiedHunks(
  oldLines: string[],
  hunks: DiffHunk[],
  contextLines: number
): string {
  // Build "change regions" with context, then merge overlapping ones
  const regions = hunks.map((hunk) => ({
    // Context-expanded range in old lines
    ctxOldStart: Math.max(0, hunk.baseStart - contextLines),
    ctxOldEnd: Math.min(
      oldLines.length,
      hunk.baseStart + hunk.baseCount + contextLines
    ),
    hunk,
  }))

  // Merge overlapping regions
  const merged: {
    ctxOldStart: number
    ctxOldEnd: number
    hunks: DiffHunk[]
  }[] = []

  for (const region of regions) {
    const last = merged[merged.length - 1]
    if (last && region.ctxOldStart <= last.ctxOldEnd) {
      // Overlapping — extend and add hunk
      last.ctxOldEnd = Math.max(last.ctxOldEnd, region.ctxOldEnd)
      last.hunks.push(region.hunk)
    } else {
      merged.push({
        ctxOldStart: region.ctxOldStart,
        ctxOldEnd: region.ctxOldEnd,
        hunks: [region.hunk],
      })
    }
  }

  // Render each merged region as a unified hunk
  const output: string[] = []
  // Running (additions − deletions) from every earlier hunk. Groups are emitted
  // in old-file order, so each hunk's `+start` is its old start shifted by the
  // net line delta of all preceding hunks — without this, later hunks report
  // wrong new-file line numbers after an earlier insertion or deletion.
  let newLineDelta = 0

  for (const group of merged) {
    const lines: string[] = []
    let oldCursor = group.ctxOldStart
    let newLineCount = 0
    let groupDelta = 0
    const oldLineCount = group.ctxOldEnd - group.ctxOldStart

    for (const hunk of group.hunks) {
      // Context lines before this change
      while (oldCursor < hunk.baseStart) {
        lines.push(` ${oldLines[oldCursor]}`)
        newLineCount++
        oldCursor++
      }

      // Deleted lines
      for (let i = 0; i < hunk.baseCount; i++) {
        lines.push(`-${oldLines[hunk.baseStart + i]}`)
        oldCursor++
      }

      // Added lines
      for (const newLine of hunk.newLines) {
        lines.push(`+${newLine}`)
        newLineCount++
      }

      groupDelta += hunk.newLines.length - hunk.baseCount
    }

    // Trailing context
    while (oldCursor < group.ctxOldEnd) {
      lines.push(` ${oldLines[oldCursor]}`)
      newLineCount++
      oldCursor++
    }

    // Compute hunk header
    const oldStart = oldLineCount === 0 ? 0 : group.ctxOldStart + 1
    const newStart =
      newLineCount === 0 ? 0 : group.ctxOldStart + 1 + newLineDelta

    output.push(
      `@@ -${oldStart},${oldLineCount} +${newStart},${newLineCount} @@`
    )
    output.push(...lines)

    newLineDelta += groupDelta
  }

  return output.join("\n")
}

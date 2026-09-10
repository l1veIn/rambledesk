// Adapted from xintaofei/codeg, commit 3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1.
// SPDX-License-Identifier: Apache-2.0
// RambleDesk: pure diff logic retained; imports point to the local extracted engine.
export interface LineChangeStats {
  additions: number
  deletions: number
}

/**
 * Maximum product of changed-window line counts before both the collapsed stat
 * (the LIS below) and the expanded diff (`generateUnifiedDiff`'s LCS table)
 * stop computing an exact diff and treat the whole window as changed.
 *
 * This single ceiling is the crux of keeping the two consistent: because the
 * number of matching line pairs is ≤ the product of the line counts, one
 * product budget bounds BOTH the LIS work here and the O(n*m) diff table, and —
 * decisively — makes the two share ONE fallback trigger. Below the budget both
 * compute the exact line LCS (identical counts); at or above it both report the
 * full trimmed window (identical counts). They can therefore never disagree.
 *
 * Kept modest because the collapsed stat runs on every edit-card render;
 * localized edits trim to a tiny window regardless of file size, so only
 * genuinely huge contiguous rewrites (~1000+ changed lines) reach the fallback.
 */
export const LINE_DIFF_LCS_BUDGET = 1_000_000

/**
 * Whether a changed window is too large for an exact line LCS. Shared by the
 * collapsed stat and `generateUnifiedDiff` so they fall back in lockstep.
 */
export function exceedsLineDiffBudget(
  oldWindow: string[],
  newWindow: string[]
): boolean {
  return oldWindow.length * newWindow.length > LINE_DIFF_LCS_BUDGET
}

export function splitNormalizedLines(text: string): string[] {
  if (!text) return []
  const lines = text.split("\n")
  if (lines.length > 0 && lines[lines.length - 1] === "") {
    lines.pop()
  }
  return lines
}

/**
 * Strip the common leading/trailing lines shared by both sides, returning just
 * the changed window. Mirrors the trimming in `generateUnifiedDiff` so the
 * collapsed stat and the expanded diff are computed over the same region.
 */
function trimCommonOuterLines(
  oldLines: string[],
  newLines: string[]
): { oldWindow: string[]; newWindow: string[] } {
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

  return {
    oldWindow: oldLines.slice(prefix, oldLines.length - suffix),
    newWindow: newLines.slice(prefix, newLines.length - suffix),
  }
}

function lowerBound(values: number[], target: number): number {
  let left = 0
  let right = values.length
  while (left < right) {
    const mid = left + ((right - left) >> 1)
    if (values[mid] < target) {
      left = mid + 1
    } else {
      right = mid
    }
  }
  return left
}

function lcsLengthByLine(oldLines: string[], newLines: string[]): number {
  const positions = new Map<string, number[]>()
  for (let i = 0; i < newLines.length; i += 1) {
    const line = newLines[i]
    const bucket = positions.get(line)
    if (bucket) {
      bucket.push(i)
    } else {
      positions.set(line, [i])
    }
  }

  const lis: number[] = []
  for (const line of oldLines) {
    const bucket = positions.get(line)
    if (!bucket || bucket.length === 0) continue

    for (let i = bucket.length - 1; i >= 0; i -= 1) {
      const pos = bucket[i]
      const at = lowerBound(lis, pos)
      if (at === lis.length) {
        lis.push(pos)
      } else {
        lis[at] = pos
      }
    }
  }

  return lis.length
}

export function estimateChangedLineStats(
  oldText: string,
  newText: string
): LineChangeStats {
  const oldLines = splitNormalizedLines(oldText)
  const newLines = splitNormalizedLines(newText)

  if (oldLines.length === 0 && newLines.length === 0) {
    return { additions: 0, deletions: 0 }
  }
  if (oldLines.length === 0) {
    return { additions: newLines.length, deletions: 0 }
  }
  if (newLines.length === 0) {
    return { additions: 0, deletions: oldLines.length }
  }

  // Trim the common prefix/suffix first so this estimate runs on the same
  // changed window the unified diff renders from (generateUnifiedDiff trims
  // identically). For the normal LCS path this is mathematically equal to
  // diffing the full text — common outer lines belong to every LCS — but it
  // keeps the collapsed +N/−M stat in agreement with the expanded diff when the
  // pair-budget fallback would otherwise diverge (e.g. a large duplicate prefix
  // that the diff trims away before counting).
  const { oldWindow, newWindow } = trimCommonOuterLines(oldLines, newLines)
  if (oldWindow.length === 0) {
    return { additions: newWindow.length, deletions: 0 }
  }
  if (newWindow.length === 0) {
    return { additions: 0, deletions: oldWindow.length }
  }

  // Window too large / duplicate-heavy for an exact LCS: report the whole
  // (already-trimmed) window as changed. generateUnifiedDiff applies the SAME
  // gate, so the collapsed stat and the expanded diff fall back together and
  // their +/- counts stay identical.
  if (exceedsLineDiffBudget(oldWindow, newWindow)) {
    return { additions: newWindow.length, deletions: oldWindow.length }
  }

  const lcs = lcsLengthByLine(oldWindow, newWindow)
  return {
    additions: Math.max(0, newWindow.length - lcs),
    deletions: Math.max(0, oldWindow.length - lcs),
  }
}

const UNIFIED_HUNK_HEADER = /^@@ -\d+(?:,(\d+))? \+\d+(?:,(\d+))? @@/

export function countUnifiedDiffLineChanges(text: string): LineChangeStats {
  const lines = text.split("\n")

  // Precise path for real unified diffs: walk each `@@` hunk and consume exactly
  // the number of old/new lines it declares. This attributes every body line by
  // POSITION, so a deletion/addition whose content itself begins with `--- `/
  // `+++ ` (source that starts with `-- `/`++ `, emitted as `--- `/`+++ `) is
  // counted correctly instead of being mistaken for a `--- a/…`/`+++ b/…` file
  // header — which a pure prefix scan cannot disambiguate.
  if (lines.some((line) => UNIFIED_HUNK_HEADER.test(line))) {
    let additions = 0
    let deletions = 0
    let i = 0
    while (i < lines.length) {
      const header = UNIFIED_HUNK_HEADER.exec(lines[i])
      i += 1
      if (!header) continue

      // Omitted count means 1 (`@@ -a +c @@`).
      let oldRemaining = header[1] === undefined ? 1 : Number(header[1])
      let newRemaining = header[2] === undefined ? 1 : Number(header[2])
      while (i < lines.length && (oldRemaining > 0 || newRemaining > 0)) {
        const line = lines[i]
        i += 1
        if (line.startsWith("\\")) continue // "\ No newline at end of file"
        if (line.startsWith("-")) {
          deletions += 1
          oldRemaining -= 1
        } else if (line.startsWith("+")) {
          additions += 1
          newRemaining -= 1
        } else {
          // Context line (leading space); belongs to both sides.
          oldRemaining -= 1
          newRemaining -= 1
        }
      }
    }
    return { additions, deletions }
  }

  // Header-less +/- block (e.g. a new-file diff without `@@` hunks): prefix scan,
  // skipping the `--- a/…` / `+++ b/…` file-header lines (which carry a trailing
  // space before the path).
  let additions = 0
  let deletions = 0
  for (const line of lines) {
    if (line.startsWith("+") && !/^\+\+\+ /.test(line)) additions += 1
    if (line.startsWith("-") && !/^--- /.test(line)) deletions += 1
  }
  return { additions, deletions }
}

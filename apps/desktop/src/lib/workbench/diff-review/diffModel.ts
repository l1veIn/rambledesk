import { diffLines, type DiffLine } from '../../agents/chat/diff-lines'
import type { DiffReviewAnchor, DiffReviewFile } from '../../generated/feedback'

type LineRange = { start: number; count: number }
export type ReviewDiffHunk = { index: number; header: string; old: LineRange; new: LineRange; lines: DiffLine[] }
const MAX_LINE = 0xffffffff
const metadata = ['diff --git ', 'index ', '--- ', '+++ ', 'old mode ', 'new mode ', 'deleted file mode ',
  'new file mode ', 'similarity index ', 'dissimilarity index ', 'rename from ', 'rename to ', 'copy from ', 'copy to ']
const noNewline = '\\ No newline at end of file'

function range(start: string, count = '1'): LineRange | null {
  const value = { start: Number(start), count: Number(count) }
  return Number.isInteger(value.start) && Number.isInteger(value.count) && value.start + value.count <= MAX_LINE
    && (value.count === 0 || value.start > 0) ? value : null
}

/** Exact counts mirror the immutable-input validator; display parsing never authorizes an anchor. */
export function parseReviewDiff(diff: string): ReviewDiffHunk[] | null {
  const source = diff.split('\n')
  if (source.at(-1) === '') source.pop()
  const hunks: (Omit<ReviewDiffHunk, 'lines'> & { body: string[] })[] = []
  let consumed = { old: 0, new: 0 }, previousBody = false
  const headers = [0, 0, 0]
  for (const raw of source) {
    const line = raw.endsWith('\r') ? raw.slice(0, -1) : raw
    const previous = hunks.at(-1)
    if (line.startsWith('@@')) {
      if (previous && (consumed.old !== previous.old.count || consumed.new !== previous.new.count)) return null
      const match = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/.exec(line)
      const old = match && range(match[1], match[2]), next = match && range(match[3], match[4])
      if (!old || !next || old.count + next.count === 0) return null
      if (previous && (old.start < previous.old.start + previous.old.count || next.start < previous.new.start + previous.new.count)) return null
      hunks.push({ index: hunks.length, header: line, old, new: next, body: [] })
      consumed = { old: 0, new: 0 }; previousBody = false
    } else if (line === noNewline) {
      if (!previous || !previousBody) return null
      previous.body.push(line); previousBody = false
    } else if (previous) {
      if (consumed.old === previous.old.count && consumed.new === previous.new.count) {
        if (line !== '') return null
        previousBody = false
        continue
      }
      if (line[0] === ' ') { consumed.old += 1; consumed.new += 1 }
      else if (line[0] === '-') consumed.old += 1
      else if (line[0] === '+') consumed.new += 1
      else return null
      if (consumed.old > previous.old.count || consumed.new > previous.new.count) return null
      previous.body.push(line); previousBody = true
    } else {
      for (const [index, prefix] of ['diff --git ', '--- ', '+++ '].entries()) if (line.startsWith(prefix)) headers[index] += 1
      if (headers.some((count) => count > 1) || (line !== '' && !metadata.some((prefix) => line.startsWith(prefix)))) return null
    }
  }
  const last = hunks.at(-1)
  if (!last || consumed.old !== last.old.count || consumed.new !== last.new.count) return null
  return hunks.map(({ body, ...hunk }) => {
    // The shared display parser does not recognize no-newline metadata. Preserve
    // that marker visibly without letting it shift the following source numbers.
    const rows = diffLines([hunk.header, ...body.filter((line) => line !== noNewline)].join('\n')).slice(1)
    let index = 0
    return { ...hunk, lines: body.map((text) => text === noNewline
      ? { kind: 'header' as const, text, oldLine: null, newLine: null } : rows[index++]) }
  })
}

export function diffFileMetadata(diff: string): DiffLine[] {
  const source = diff.split('\n').map((line) => line.endsWith('\r') ? line.slice(0, -1) : line)
  const first = source.findIndex((line) => line.startsWith('@@'))
  return first > 0 ? diffLines(source.slice(0, first).join('\n')) : []
}

export function validDiffAnchor(file: DiffReviewFile, anchor: DiffReviewAnchor, hunks = parseReviewDiff(file.diff)): boolean {
  const hunk = hunks?.[anchor.hunk_index]
  if (!hunk || file.id !== anchor.file_id || (anchor.side !== 'old' && anchor.side !== 'new')) return false
  if (anchor.start_line === null || anchor.end_line === null) return anchor.start_line === null && anchor.end_line === null
  const side = hunk[anchor.side]
  return Number.isInteger(anchor.start_line) && Number.isInteger(anchor.end_line) && anchor.start_line >= 1
    && anchor.end_line >= anchor.start_line && anchor.start_line >= side.start && anchor.end_line < side.start + side.count
}

export const diffFileLabel = (file: DiffReviewFile) => file.new_path === '/dev/null' ? file.old_path : file.new_path
export function diffAnchorLabel(file: DiffReviewFile | undefined, anchor: DiffReviewAnchor): string {
  const path = [...(file ? diffFileLabel(file) : anchor.file_id)].slice(-160).join('')
  return `${path} · @@${anchor.hunk_index + 1}${anchor.start_line === null ? ''
    : ` · ${anchor.side === 'old' ? '−' : '+'}${anchor.start_line}${anchor.end_line !== anchor.start_line ? `–${anchor.end_line}` : ''}`}`
}
export function sameDiffAnchor(first: DiffReviewAnchor, second: DiffReviewAnchor): boolean {
  return first.file_id === second.file_id && first.hunk_index === second.hunk_index && first.side === second.side
    && first.start_line === second.start_line && first.end_line === second.end_line
}
export function anchorIncludesLine(anchor: DiffReviewAnchor | null, hunk: number, side: 'old' | 'new', line: number): boolean {
  return !!anchor && anchor.hunk_index === hunk && (anchor.start_line === null || (anchor.side === side
    && line >= anchor.start_line && line <= anchor.end_line!))
}

export function diffAnchorOffset(hunks: readonly ReviewDiffHunk[], anchor: DiffReviewAnchor): number {
  let offset = 0
  for (const hunk of hunks) {
    if (hunk.index === anchor.hunk_index) {
      const index = anchor.start_line === null ? 0 : hunk.lines.findIndex((line) => line[anchor.side === 'old' ? 'oldLine' : 'newLine'] === anchor.start_line)
      return offset + Math.max(0, index)
    }
    offset += hunk.lines.length
  }
  return 0
}
export function diffWindow(hunks: readonly ReviewDiffHunk[], start: number, size: number): ReviewDiffHunk[] {
  let offset = 0
  return hunks.flatMap((hunk) => {
    const from = Math.max(0, start - offset), to = Math.min(hunk.lines.length, start + size - offset)
    offset += hunk.lines.length
    return from < to ? [{ ...hunk, lines: hunk.lines.slice(from, to) }] : []
  })
}

export type SplitDiffRow = { old: DiffLine | null; new: DiffLine | null }

/** Pair adjacent changed runs for display; keep each side's immutable source coordinates. */
export function splitDiffRows(lines: readonly DiffLine[]): SplitDiffRow[] {
  const rows: SplitDiffRow[] = []
  let removed: DiffLine[] = [], added: DiffLine[] = []
  function flush() {
    for (let index = 0; index < Math.max(removed.length, added.length); index += 1) {
      rows.push({ old: removed[index] ?? null, new: added[index] ?? null })
    }
    removed = []; added = []
  }
  for (const line of lines) {
    if (line.kind === 'deletion') removed.push(line)
    else if (line.kind === 'addition') added.push(line)
    else { flush(); rows.push({ old: line, new: line }) }
  }
  flush()
  return rows
}

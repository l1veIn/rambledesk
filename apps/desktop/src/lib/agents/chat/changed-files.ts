// Per-turn changed-file summary for the agent chat.
// Ported idea from Codeg src/components/message/reply-artifacts.tsx at 3ebdfed
// (Apache-2.0). RambleDesk reads durable ACP tool content and locations instead
// of re-parsing raw tool-input previews, so a bounded snapshot is enough.
import type { SessionContentBlock, SessionToolCall } from '$lib/generated/feedback'
import type { SessionActivity } from '../managedSessionUi'
import { activityTool } from './activity-presentation'
import { countUnifiedDiffLineChanges } from './line-change-stats'
import { generateUnifiedDiff } from './unified-diff-generator'

export type ChangedFileKind = 'added' | 'modified' | 'removed'

export type ChangedFile = Readonly<{
  /** Stable across re-renders: the normalized path. */
  id: string
  path: string
  kind: ChangedFileKind
  additions: number
  deletions: number
  /** Unified diff text; empty when the agent reported the file without content. */
  diff: string
}>

/** Tool kinds whose subject is a file edit; other kinds only count with diff content. */
const EDIT_KINDS = new Set(['edit', 'delete', 'move'])
const PATH_KEYS = ['file_path', 'filePath', 'path', 'target_file', 'targetFile', 'filename'] as const
const NESTED_KEYS = ['input', 'arguments', 'params', 'payload'] as const
const MAX_PATH_DEPTH = 3

type Accumulated = {
  path: string
  removed: boolean
  added: boolean
  additions: number
  deletions: number
  diffs: string[]
}

function normalizePath(path: string): string {
  return path.replace(/\\/g, '/').trim()
}

function diffBlockKind(block: Extract<SessionContentBlock, { type: 'diff' }>, tool: SessionToolCall): ChangedFileKind {
  if (tool.kind === 'delete') return 'removed'
  const oldText = block.old_text ?? ''
  if (!oldText && block.new_text) return 'added'
  if (oldText && !block.new_text && tool.kind === 'move') return 'modified'
  return 'modified'
}

function accumulateDiffBlock(accumulated: Accumulated, block: Extract<SessionContentBlock, { type: 'diff' }>, kind: ChangedFileKind) {
  const diff = generateUnifiedDiff(block.old_text ?? '', block.new_text, accumulated.path)
  if (diff) {
    const stats = countUnifiedDiffLineChanges(diff)
    accumulated.additions += stats.additions
    accumulated.deletions += stats.deletions
    accumulated.diffs.push(diff)
  }
  accumulated.removed ||= kind === 'removed'
  accumulated.added ||= kind === 'added'
}

/**
 * Bounded fallback for agents that report an edit path without diff content:
 * read the first path-shaped field from the stored input preview.
 */
export function pathFromRawInput(raw: string | null): string | null {
  if (!raw) return null
  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return null
  }
  return findPath(parsed, 0)
}

function findPath(value: unknown, depth: number): string | null {
  if (depth > MAX_PATH_DEPTH || value === null || typeof value !== 'object') return null
  if (Array.isArray(value)) {
    for (const item of value) {
      const found = findPath(item, depth + 1)
      if (found) return found
    }
    return null
  }
  const record = value as Record<string, unknown>
  for (const key of PATH_KEYS) {
    const candidate = record[key]
    if (typeof candidate === 'string' && candidate.trim()) return candidate
  }
  for (const key of NESTED_KEYS) {
    const found = findPath(record[key], depth + 1)
    if (found) return found
  }
  return null
}

function pathsForTool(tool: SessionToolCall): string[] {
  const paths = tool.locations.map((location) => location.path).filter(Boolean)
  if (paths.length) return paths
  const fallback = pathFromRawInput(tool.raw_input)
  return fallback ? [fallback] : []
}

function changedByTool(activity: SessionActivity, byPath: Map<string, Accumulated>, order: string[]) {
  const tool = activityTool(activity)
  if (!tool) return

  const ensure = (path: string): Accumulated => {
    const normalized = normalizePath(path)
    let accumulated = byPath.get(normalized)
    if (!accumulated) {
      accumulated = { path: normalized, removed: false, added: false, additions: 0, deletions: 0, diffs: [] }
      byPath.set(normalized, accumulated)
      order.push(normalized)
    }
    return accumulated
  }

  const diffBlocks = tool.content.filter(
    (block): block is Extract<SessionContentBlock, { type: 'diff' }> =>
      block.type === 'diff' && Boolean(block.path),
  )
  for (const block of diffBlocks) {
    accumulateDiffBlock(ensure(block.path), block, diffBlockKind(block, tool))
  }

  // Diff content is authoritative for this call: its `locations` may repeat the
  // same file in another path form, which would list it twice. A file the agent
  // touched without reporting a diff still belongs in the list.
  if (diffBlocks.length === 0 && EDIT_KINDS.has(tool.kind)) {
    for (const path of pathsForTool(tool)) {
      const accumulated = ensure(path)
      accumulated.removed ||= tool.kind === 'delete'
    }
  }
}

/**
 * One entry per file the turn wrote, summed across every tool call and in
 * first-seen order. A file created and then edited stays "added"; a file that
 * was also deleted is "removed".
 */
export function turnChangedFiles(activities: readonly SessionActivity[]): ChangedFile[] {
  const byPath = new Map<string, Accumulated>()
  const order: string[] = []
  for (const activity of activities) changedByTool(activity, byPath, order)
  return order.map((path) => {
    const accumulated = byPath.get(path)!
    return {
      id: path,
      path: accumulated.path,
      kind: accumulated.removed ? 'removed' : accumulated.added ? 'added' : 'modified',
      additions: accumulated.additions,
      deletions: accumulated.deletions,
      diff: accumulated.diffs.join('\n\n'),
    }
  })
}

/** Trailing path segment, used for tab titles and card headlines. */
export function fileNameOf(path: string): string {
  const normalized = normalizePath(path)
  return normalized.slice(normalized.lastIndexOf('/') + 1) || normalized
}

/** Directory portion of a path, or an empty string when the path is a bare name. */
export function directoryOf(path: string): string {
  const normalized = normalizePath(path)
  const index = normalized.lastIndexOf('/')
  return index <= 0 ? '' : normalized.slice(0, index)
}

/** Prefer a session-relative path so absolute agent paths stay readable. */
export function displayPath(path: string, cwd: string): string {
  const normalized = normalizePath(path)
  if (!cwd) return normalized
  const root = normalizePath(cwd).replace(/\/+$/, '')
  if (!root || !normalized.startsWith(`${root}/`)) return normalized
  return normalized.slice(root.length + 1)
}

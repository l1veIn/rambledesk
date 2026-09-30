import { decodeFeedbackDraftEnvelope, type FeedbackDraftSnapshot } from '../feedbackDraftDocument'
import { decodeWorkbenchState } from '../workbenchPolicy'
import { sameWorkbenchFieldIdentity } from '../workbenchFields'
import { captureFieldTidy, replaceFieldTidy, type FieldTidyWorkspace } from './fieldSpeechText'
import { normalizeSpeechTarget, sameSpeechTarget, snapshotSpeechTarget, type SpeechTarget } from './speechTargets'
import type { SpeechCleanupReplacement } from './speechBlockMetadata'
import type { SpeechWriteInput } from './speechWriteback'

export type FieldSpeechSegment = {
  segmentId: string
  target: SpeechTarget
  /** Offsets count Unicode scalars, like document-review source anchors. */
  start: number
  end: number
  text: string
  /** Edited spans remain as retired provenance, even if their text is restored. */
  state: 'pending' | 'cleaned' | 'edited'
  /** The selected question or original paragraph at capture time. */
  contract: string
  /** The custom-answer identity or review anchor, excluding mutable text. */
  identity: string
}
export type FieldSpeechCleanupReplacement = SpeechCleanupReplacement & { target: SpeechTarget }
const record = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value)
const snapshotOf = (workspace: FieldTidyWorkspace): FeedbackDraftSnapshot => ({ documentJson: workspace.draft.document_json ?? '', bodyMarkdown: workspace.draft.body_markdown })
const withSnapshot = (workspace: FieldTidyWorkspace, snapshot: FeedbackDraftSnapshot): FieldTidyWorkspace => ({ ...workspace,
  draft: { document_json: snapshot.documentJson, body_markdown: snapshot.bodyMarkdown } })

/** Unsupported metadata is preserved, never interpreted or overwritten. */
function readSegments(snapshot: FeedbackDraftSnapshot): FieldSpeechSegment[] | null {
  const value = decodeFeedbackDraftEnvelope(snapshot.documentJson)?.fieldSpeechSegments
  if (value === undefined) return []
  if (!record(value) || value.version !== 1 || !Array.isArray(value.segments)) return null
  const segments: FieldSpeechSegment[] = []
  const seen = new Set<string>()
  for (const item of value.segments) {
    if (!record(item) || typeof item.segmentId !== 'string' || !item.segmentId || seen.has(item.segmentId)
      || typeof item.text !== 'string' || !Number.isInteger(item.start) || !Number.isInteger(item.end)
      || (item.start as number) < 0 || (item.end as number) < (item.start as number)
      || !['pending', 'cleaned', 'edited'].includes(String(item.state)) || typeof item.contract !== 'string' || typeof item.identity !== 'string') return null
    const target = normalizeSpeechTarget(item.target)
    if (!target || (target.destination.kind !== 'question_answer' && target.destination.kind !== 'review_annotation'
      && target.destination.kind !== 'web_review_annotation')) return null
    seen.add(item.segmentId)
    segments.push({ ...item, target } as FieldSpeechSegment)
  }
  return segments
}

function writeSegments(snapshot: FeedbackDraftSnapshot, segments: FieldSpeechSegment[]): FeedbackDraftSnapshot {
  const envelope = decodeFeedbackDraftEnvelope(snapshot.documentJson)
  if (!envelope) return snapshot
  return { ...snapshot, documentJson: JSON.stringify({ ...envelope, fieldSpeechSegments: { version: 1, segments } }) }
}

function fieldText(snapshot: FeedbackDraftSnapshot, target: SpeechTarget): string | null {
  const state = decodeWorkbenchState(decodeFeedbackDraftEnvelope(snapshot.documentJson)?.workbenchState)
  const destination = target.destination
  if (destination.kind === 'question_answer' && state?.type === 'questions') {
    const answers = state.answers.filter((item) => item.id === destination.questionId)
    return answers.length === 1 && answers[0].wasCustom ? answers[0].value : null
  }
  if (destination.kind === 'review_annotation' && state?.type === 'document_review') {
    const annotations = state.annotations.filter((item) => item.id === destination.annotationId)
    return annotations.length === 1 && (destination.field === 'body' || annotations[0].kind === 'suggestion')
      ? annotations[0][destination.field] : null
  }
  if (destination.kind === 'web_review_annotation' && state?.type === 'web_review') {
    const annotations = state.annotations.filter((item) => item.id === destination.annotationId)
    return annotations.length === 1 ? annotations[0].body : null
  }
  return null
}

/** The smallest single changed range is conservative for multi-part edits:
 * untouched prefixes/suffixes retain provenance; everything between is edited. */
function reconcileSegmentText(segment: FieldSpeechSegment, before: string | null, after: string | null): FieldSpeechSegment {
  if (before === after) return segment
  const edited: FieldSpeechSegment = { ...segment, state: 'edited' }
  if (before === null || after === null) return edited
  const oldText = [...before], newText = [...after]
  if (oldText.slice(segment.start, segment.end).join('') !== segment.text) return edited

  let prefix = 0
  while (prefix < oldText.length && prefix < newText.length && oldText[prefix] === newText[prefix]) prefix += 1
  let suffix = 0
  while (suffix < oldText.length - prefix && suffix < newText.length - prefix
    && oldText[oldText.length - suffix - 1] === newText[newText.length - suffix - 1]) suffix += 1
  if (segment.end <= prefix) return segment
  if (segment.start < oldText.length - suffix) return edited
  const delta = newText.length - oldText.length
  return { ...segment, start: segment.start + delta, end: segment.end + delta }
}

/** Cleanup changes one known span. Keep later offsets in the same field aligned,
 * and retire overlapping provenance rather than attributing model text to it. */
function segmentsAfterCleanup(segments: FieldSpeechSegment[], cleaned: FieldSpeechSegment, nextText: string): FieldSpeechSegment[] {
  const length = [...nextText].length
  const delta = length - (cleaned.end - cleaned.start)
  return segments.map((segment) => {
    if (segment.segmentId === cleaned.segmentId) return { ...segment, text: nextText, end: segment.start + length, state: 'cleaned' }
    if (segment.state === 'edited' || !sameSpeechTarget(segment.target, cleaned.target) || segment.end <= cleaned.start) return segment
    if (segment.start >= cleaned.end) return { ...segment, start: segment.start + delta, end: segment.end + delta }
    return { ...segment, state: 'edited' }
  })
}

/** Called only after a new speech receipt has been accepted in the same draft write. */
export function recordFieldSpeechSegment(workspace: FieldTidyWorkspace, snapshot: FeedbackDraftSnapshot,
  input: SpeechWriteInput, previousText: string): FeedbackDraftSnapshot {
  const segments = readSegments(snapshot)
  if (!segments || segments.some((item) => item.segmentId === input.id)) return snapshot
  const capture = captureFieldTidy(withSnapshot(workspace, snapshot), input)
  const prefix = previousText + (previousText && !/\s$/.test(previousText) ? '\n' : '')
  const start = [...prefix].length
  const segment: FieldSpeechSegment = { segmentId: input.id, target: snapshotSpeechTarget(input), start,
    end: start + [...input.text].length, text: input.text, state: input.cleanupState === 'cleaned' ? 'cleaned' : 'pending',
    contract: capture.contract, identity: capture.identity }
  if ([...capture.original].slice(segment.start, segment.end).join('') !== segment.text) return snapshot
  return writeSegments(snapshot, [...segments, segment])
}

/** Read only provenance that still names an editable, unchanged field span. */
export function collectFieldSpeechSegments(workspace: FieldTidyWorkspace): FieldSpeechSegment[] {
  return (readSegments(snapshotOf(workspace)) ?? []).filter((segment) => {
    if (segment.state !== 'pending' || !segment.text.trim()) return false
    try {
      const capture = captureFieldTidy(workspace, segment.target)
      return capture.contract === segment.contract && sameWorkbenchFieldIdentity(segment.identity, capture.identity, segment.target)
        && [...capture.original].slice(segment.start, segment.end).join('') === segment.text
    } catch { return false }
  })
}

/** Provenance for the current field view; stale or manually edited text is never marked. */
export function fieldSpeechSegmentsForTarget(snapshot: FeedbackDraftSnapshot, target: SpeechTarget): FieldSpeechSegment[] {
  const text = fieldText(snapshot, target)
  if (text === null) return []
  const characters = [...text]
  return (readSegments(snapshot) ?? []).filter((segment) => segment.state !== 'edited'
    && sameSpeechTarget(segment.target, target)
    && characters.slice(segment.start, segment.end).join('') === segment.text)
}

/** Field-local counts use the same valid spans displayed by the input editor. */
export function fieldSpeechSegmentCounts(snapshot: FeedbackDraftSnapshot, target: SpeechTarget): { pending: number; cleaned: number } {
  const result = { pending: 0, cleaned: 0 }
  for (const segment of fieldSpeechSegmentsForTarget(snapshot, target)) {
    if (segment.state !== 'edited') result[segment.state] += 1
  }
  return result
}

/** One conservative edit range: outside spans move; touched spans become ordinary edited text. */
export function reconcileFieldSpeechSegments(previous: FeedbackDraftSnapshot, next: FeedbackDraftSnapshot): FeedbackDraftSnapshot {
  const segments = readSegments(previous)
  if (!segments?.length || readSegments(next) === null) return next
  let changed = false
  const reconciled = segments.map((segment) => {
    if (segment.state === 'edited') return segment
    const before = fieldText(previous, segment.target)
    const after = fieldText(next, segment.target)
    if (before !== after) changed = true
    return reconcileSegmentText(segment, before, after)
  })
  return changed ? writeSegments(next, reconciled) : next
}

/** Apply independently guarded replacements to one complete draft snapshot. */
export function applyFieldSpeechCleanup(workspace: FieldTidyWorkspace, replacements: FieldSpeechCleanupReplacement[]): {
  snapshot: FeedbackDraftSnapshot; applied: string[]; skipped: string[]
} {
  let snapshot = snapshotOf(workspace)
  const applied: string[] = [], skipped: string[] = []
  const ids = new Set<string>()
  for (const replacement of replacements) {
    const latest = withSnapshot(workspace, snapshot)
    const segment = collectFieldSpeechSegments(latest).find((item) => item.segmentId === replacement.segmentId
      && sameSpeechTarget(item.target, replacement.target) && item.text === replacement.originalText)
    if (!segment || ids.has(replacement.segmentId)) { skipped.push(replacement.segmentId); continue }
    ids.add(replacement.segmentId)
    try {
      const capture = captureFieldTidy(latest, segment.target)
      const source = [...capture.original]
      const value = source.slice(0, segment.start).join('') + replacement.nextText + source.slice(segment.end).join('')
      const next = replaceFieldTidy(latest, capture, capture.original, value)
      const segments = segmentsAfterCleanup(readSegments(snapshot)!, segment, replacement.nextText)
      snapshot = writeSegments(next, segments)
      applied.push(replacement.segmentId)
    } catch { skipped.push(replacement.segmentId) }
  }
  return { snapshot, applied, skipped }
}

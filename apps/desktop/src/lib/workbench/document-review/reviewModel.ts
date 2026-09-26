import type { DocumentReviewData, ParagraphMark, ReviewAnnotation, WorkbenchState } from '../../generated/feedback'

export type DocumentReviewState = Extract<WorkbenchState, { type: 'document_review' }>
export type ReviewAnchor = Pick<ReviewAnnotation, 'paragraph_id' | 'start' | 'end' | 'quote'>
const stableId = /^[a-z0-9][a-z0-9_-]{0,63}$/
const textWithin = (value: unknown, max: number, required = false): value is string =>
  typeof value === 'string' && !value.includes('\0') && [...value].length <= max && (!required || value.trim().length > 0)

export function emptyReviewState(): DocumentReviewState {
  return { type: 'document_review', verdict: null, annotations: [], paragraph_marks: [] }
}

export function hasReviewInput(_data: DocumentReviewData, state: DocumentReviewState | null): boolean {
  return !!state && (state.verdict === 'ready' || state.verdict === 'changes_requested'
    || (Array.isArray(state.annotations) && state.annotations.some((item) => typeof item?.body === 'string' && item.body.trim()))
    || (Array.isArray(state.paragraph_marks) && state.paragraph_marks.length > 0))
}

/** Submission validation. Incomplete comments remain valid editable drafts. */
export function validateReviewState(data: DocumentReviewData, state: DocumentReviewState | null): string | null {
  if (!state || !['ready', 'changes_requested'].includes(state.verdict ?? '')) return 'Choose a review decision.'
  if (!Array.isArray(state.annotations) || state.annotations.length > 500 || !Array.isArray(state.paragraph_marks)) return 'Invalid review state.'
  const paragraphs = new Map(data.paragraphs.map((paragraph) => [paragraph.id, paragraph]))
  const ids = new Set<string>()
  for (const annotation of state.annotations) {
    if (!annotation || typeof annotation.id !== 'string' || !stableId.test(annotation.id) || ids.has(annotation.id)) return 'Invalid or duplicate comment id.'
    ids.add(annotation.id)
    const paragraph = paragraphs.get(annotation.paragraph_id)
    if (!paragraph) return 'A comment refers to an unavailable paragraph.'
    if (!textWithin(annotation.body, 4000, true)) return 'Finish or remove empty comments.'
    if (annotation.kind === 'suggestion') {
      if (!textWithin(annotation.replacement, 8000)) return 'Enter the suggested wording.'
    } else if (annotation.kind !== 'comment' || annotation.replacement !== null) return 'Invalid comment kind.'
    if (annotation.start === null && annotation.end === null && annotation.quote === null) continue
    const text = [...paragraph.text]
    if (!Number.isInteger(annotation.start) || !Number.isInteger(annotation.end)
      || annotation.start === null || annotation.end === null || annotation.start < 0
      || annotation.end <= annotation.start || annotation.end > text.length
      || text.slice(annotation.start, annotation.end).join('') !== annotation.quote) return 'A comment no longer matches the original text.'
  }
  const marked = new Set<string>()
  for (const mark of state.paragraph_marks) {
    if (!mark || !paragraphs.has(mark.paragraph_id) || marked.has(mark.paragraph_id)
      || !['keep', 'revise', 'remove'].includes(mark.decision)) return 'Invalid paragraph mark.'
    marked.add(mark.paragraph_id)
  }
  return null
}

export function changeParagraphMark(state: DocumentReviewState, paragraphId: string, decision: ParagraphMark['decision'] | ''): DocumentReviewState {
  return { ...state, paragraph_marks: [
    ...state.paragraph_marks.filter((mark) => mark.paragraph_id !== paragraphId),
    ...(decision ? [{ paragraph_id: paragraphId, decision }] : []),
  ] }
}

export function toggleParagraphRemoval(state: DocumentReviewState, paragraphId: string): DocumentReviewState {
  const removed = state.paragraph_marks.some((mark) => mark.paragraph_id === paragraphId && mark.decision === 'remove')
  return changeParagraphMark(state, paragraphId, removed ? '' : 'remove')
}

/** Existing drafts may have several notes; preserve them and continue the user's selected note. */
export function findParagraphAnnotation(annotations: ReviewAnnotation[], paragraphId: string, preferredId?: string | null): ReviewAnnotation | undefined {
  const paragraphAnnotations = annotations.filter((annotation) => annotation.paragraph_id === paragraphId)
  return paragraphAnnotations.find((annotation) => annotation.id === preferredId)
    ?? paragraphAnnotations[0]
}

/** Every comment entry point continues the paragraph's note without changing its original anchor. */
export function prepareParagraphAnnotation(
  data: DocumentReviewData,
  state: DocumentReviewState,
  anchor: ReviewAnchor,
  kind: ReviewAnnotation['kind'],
  idFactory: () => string,
  preferredId?: string | null,
): { annotation: ReviewAnnotation; state: DocumentReviewState; created: boolean } | null {
  const existing = findParagraphAnnotation(state.annotations, anchor.paragraph_id, preferredId)
  if (!existing && state.annotations.length >= 500) return null
  const source = existing ?? anchor
  const suggestion = kind === 'suggestion' && existing?.kind !== 'suggestion'
  if (existing && !suggestion) return { annotation: existing, state, created: false }
  const replacement = suggestion
    ? source.quote ?? data.paragraphs.find((paragraph) => paragraph.id === source.paragraph_id)?.text ?? ''
    : existing?.replacement ?? null
  const annotation: ReviewAnnotation = existing
    ? { ...existing, kind: 'suggestion', replacement }
    : { ...anchor, id: idFactory(), kind, body: '', replacement }
  return {
    annotation,
    state: { ...state, annotations: existing
      ? state.annotations.map((item) => item.id === existing.id ? annotation : item)
      : [...state.annotations, annotation] },
    created: !existing,
  }
}

export interface ReviewTextSegment { text: string; annotationIds: string[] }

/** Split at all boundaries so overlapping comments never duplicate source text. */
export function annotatedSegments(text: string, annotations: ReviewAnnotation[]): ReviewTextSegment[] {
  const characters = [...text]
  const ranges = annotations.filter((item) => item.start !== null && item.end !== null
    && item.start >= 0 && item.end > item.start && item.end <= characters.length
    && characters.slice(item.start, item.end).join('') === item.quote)
  const boundaries = [...new Set([0, characters.length, ...ranges.flatMap((item) => [item.start!, item.end!])])].sort((a, b) => a - b)
  return boundaries.slice(0, -1).map((start, index) => {
    const end = boundaries[index + 1]
    const covering = ranges.filter((item) => item.start! <= start && item.end! >= end)
    return { text: characters.slice(start, end).join(''), annotationIds: covering.map((item) => item.id) }
  })
}

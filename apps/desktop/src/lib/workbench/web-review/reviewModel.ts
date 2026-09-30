import type { WebReviewAnnotation, WebReviewData, WebReviewViewport, WorkbenchState } from '../../generated/feedback'
import { validWebReviewUrl } from '../../workbenchInputValidation'

export type WebReviewState = Extract<WorkbenchState, { type: 'web_review' }>
export type WebReviewAnchor = Pick<WebReviewAnnotation, 'page_url' | 'viewport' | 'element'>
export const WEB_REVIEW_ANNOTATION_LIMIT = 500
const stableId = /^[a-z0-9][a-z0-9_-]{0,63}$/
const textWithin = (value: unknown, max: number, required = false): value is string =>
  typeof value === 'string' && !value.includes('\0') && [...value].length <= max
  && [...value].every((char) => !/^[\uD800-\uDFFF]$/.test(char)) && (!required || /[^\p{White_Space}]/u.test(value))
const integerWithin = (value: unknown, min: number, max: number): value is number =>
  typeof value === 'number' && Number.isInteger(value) && value >= min && value <= max

export function emptyWebReviewState(): WebReviewState { return { type: 'web_review', annotations: [] } }

export const validReviewUrl = validWebReviewUrl

export function validReviewViewport(value: WebReviewViewport | null | undefined): boolean {
  return !!value && integerWithin(value.width, 240, 7680) && integerWithin(value.height, 200, 4320)
}

export function hasWebReviewInput(_data: WebReviewData, state: WebReviewState | null): boolean {
  return !!state && Array.isArray(state.annotations) && state.annotations.some((item) => typeof item?.body === 'string' && !!item.body.trim())
}

/** Empty notes are editable drafts; submission requires their comments to be complete. */
export function validateWebReviewState(_data: WebReviewData, state: WebReviewState | null): string | null {
  if (!state) return null
  if (state.type !== 'web_review' || !Array.isArray(state.annotations) || state.annotations.length > WEB_REVIEW_ANNOTATION_LIMIT) return 'Invalid web review state.'
  const ids = new Set<string>()
  for (const note of state.annotations) {
    if (!note || typeof note.id !== 'string' || !stableId.test(note.id) || ids.has(note.id)) return 'Invalid or duplicate comment id.'
    ids.add(note.id)
    if (!validReviewUrl(note.page_url) || !validReviewViewport(note.viewport)) return 'Invalid page context.'
    const element = note.element
    if (!element || !textWithin(element.selector, 2000, true) || typeof element.tag_name !== 'string' || !/^[a-z][a-z0-9-]{0,63}$/.test(element.tag_name) || !textWithin(element.text, 2000)) return 'Invalid element context.'
    const rect = element.rect
    if (!rect || !integerWithin(rect.x, -1_000_000, 1_000_000) || !integerWithin(rect.y, -1_000_000, 1_000_000)
      || !integerWithin(rect.width, 1, 1_000_000) || !integerWithin(rect.height, 1, 1_000_000)) return 'Invalid element context.'
    if (!textWithin(note.body, 4000, true)) return 'Finish or remove empty comments.'
    if (note.screenshot_attachment_id != null && (typeof note.screenshot_attachment_id !== 'string' || !stableId.test(note.screenshot_attachment_id))) return 'Invalid screenshot attachment.'
  }
  return null
}

/** Reopening the same element keeps its original captured context and ongoing note. */
export function prepareElementAnnotation(
  state: WebReviewState,
  anchor: WebReviewAnchor,
  idFactory: () => string,
  preferredId?: string | null,
): { annotation: WebReviewAnnotation; state: WebReviewState; created: boolean } | null {
  const matching = state.annotations.filter((item) => item.page_url === anchor.page_url
    && item.element.selector === anchor.element.selector
    && item.viewport.width === anchor.viewport.width && item.viewport.height === anchor.viewport.height)
  const existing = matching.find((item) => item.id === preferredId) ?? matching[0]
  if (existing) return { annotation: existing, state, created: false }
  if (state.annotations.length >= WEB_REVIEW_ANNOTATION_LIMIT) return null
  const annotation: WebReviewAnnotation = {
    id: idFactory(), page_url: anchor.page_url, viewport: { ...anchor.viewport },
    element: { selector: anchor.element.selector, tag_name: anchor.element.tag_name, text: anchor.element.text, rect: { ...anchor.element.rect } }, body: '',
  }
  return { annotation, state: { ...state, annotations: [...state.annotations, annotation] }, created: true }
}

export function elementLabel(annotation: WebReviewAnnotation): string {
  return annotation.element.text.replace(/\s+/g, ' ').trim().slice(0, 80) || annotation.element.selector
}

/** Bridge selections already use document coordinates, independent of the comment panel's position. */
export function capturedElementRect(rect: WebReviewAnnotation['element']['rect']): WebReviewAnnotation['element']['rect'] {
  return { x: Math.round(rect.x), y: Math.round(rect.y), width: Math.max(1, Math.round(rect.width)), height: Math.max(1, Math.round(rect.height)) }
}

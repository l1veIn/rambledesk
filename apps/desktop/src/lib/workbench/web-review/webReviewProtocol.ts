export const WEB_REVIEW_PROTOCOL = 'rambledesk.web-review.v1'

export type WebReviewRect = Readonly<{ x: number; y: number; width: number; height: number }>
export type WebReviewViewport = Readonly<{ width: number; height: number; scroll_x: number; scroll_y: number }>
export type WebReviewSelection = Readonly<{
  page_url: string
  page_title: string
  selector: string
  tag_name: string
  text: string
  attributes: Readonly<Record<string, string>>
  rect: WebReviewRect
  viewport: WebReviewViewport
  captured_at: string
}>
export type WebReviewFrameState = Readonly<{
  status: 'loading' | 'ready' | 'unavailable'
  page_url: string
  page_title: string
  viewport: WebReviewViewport | null
  reason?: 'bridge_missing' | 'page_unavailable' | 'invalid_url'
}>
export type WebReviewMarker = Readonly<{ id: string; number: number; selector: string; page_url: string }>
export type WebReviewFocus = Readonly<{ selector: string; page_url: string; sequence: number }>

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}
function finite(value: unknown): value is number {
  return typeof value === 'number' && Number.isInteger(value) && Math.abs(value) <= 1_000_000
}
function scalarText(value: unknown, maximum: number): value is string {
  return typeof value === 'string' && !value.includes('\0') && !/\p{Surrogate}/u.test(value) && [...value].length <= maximum
}
export function validReviewRect(value: unknown): value is WebReviewRect {
  return record(value) && finite(value.x) && finite(value.y) && finite(value.width) && finite(value.height)
    && value.width >= 0 && value.height >= 0
}
export function validReviewViewport(value: unknown): value is WebReviewViewport {
  return record(value) && finite(value.width) && finite(value.height) && value.width >= 240 && value.width <= 7680 && value.height >= 200 && value.height <= 4320
    && finite(value.scroll_x) && finite(value.scroll_y)
}
export function reviewPageUrl(value: unknown): string | null {
  return validWebReviewUrl(value) ? new URL(value).href : null
}
export function validReviewSelection(value: unknown): value is WebReviewSelection {
  return record(value) && reviewPageUrl(value.page_url) !== null
    && scalarText(value.page_title, 1000)
    && scalarText(value.selector, 2000) && value.selector.trim().length > 0
    && typeof value.tag_name === 'string' && /^[a-z][a-z0-9-]{0,63}$/.test(value.tag_name)
    && scalarText(value.text, 2000)
    && record(value.attributes) && Object.entries(value.attributes).every(([key, item]) => key.length <= 100 && typeof item === 'string' && [...item].length <= 1000)
    && Object.keys(value.attributes).length <= 8 && validReviewRect(value.rect) && validReviewViewport(value.viewport)
    && value.rect.width > 0 && value.rect.height > 0
    && typeof value.captured_at === 'string' && Number.isFinite(Date.parse(value.captured_at))
}
import { validWebReviewUrl } from '../definitions/web_review/input'

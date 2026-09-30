import { object, fields, list, text, optionalText, uniqueId, type RecordValue } from '../validation'

export function validWebReviewUrl(value: unknown): value is string {
  if (!text(value, 8192) || /[\s\\\u0000-\u001f\u007f-\u009f]/.test(value) || !/^https?:\/\//i.test(value)) return false
  try {
    const url = new URL(value)
    return !!url.hostname && !url.username && !url.password
  } catch { return false }
}

export function validWebReviewInput(data: RecordValue): boolean {
  const viewport = data.viewport
  return fields(data, ['title', 'url', 'source_version', 'viewport']) && text(data.title, 200)
    && text(data.source_version, 128) && validWebReviewUrl(data.url) && object(viewport)
    && fields(viewport, ['width', 'height']) && Number.isInteger(viewport.width) && Number.isInteger(viewport.height)
    && (viewport.width as number) >= 240 && (viewport.width as number) <= 7680
    && (viewport.height as number) >= 200 && (viewport.height as number) <= 4320
}


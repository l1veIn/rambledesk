import { fields, text, type RecordValue } from '../validation'

export function validMediaReviewInput(data: RecordValue): boolean {
  if (!fields(data, ['title', 'source_version', 'media_kind', 'media_file_name', 'duration_ms'])
    || !text(data.title, 200) || !text(data.source_version, 128)
    || (data.media_kind !== 'audio' && data.media_kind !== 'video')
    || !text(data.media_file_name, 255, false)) return false
  if (typeof data.media_file_name !== 'string') return false
  const name = data.media_file_name.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, '')
  return !!name && name === data.media_file_name && new TextEncoder().encode(name).length <= 255 && !/[/\\\0]/.test(name) && name !== '.' && name !== '..'
    && Number.isInteger(data.duration_ms) && Number(data.duration_ms) >= 1 && Number(data.duration_ms) <= 86400000
}

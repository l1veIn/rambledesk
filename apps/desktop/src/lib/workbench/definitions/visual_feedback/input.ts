import { fields, text, type RecordValue } from '../validation'

export function validVisualFeedbackInput(data: RecordValue): boolean {
  const name = typeof data.image_file_name === 'string' ? data.image_file_name.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, '') : null
  return fields(data, ['title', 'source_version', 'width', 'height', 'image_file_name', 'background_color'])
    && text(data.title, 200) && text(data.source_version, 128)
    && Number.isInteger(data.width) && Number(data.width) >= 32 && Number(data.width) <= 4096
    && Number.isInteger(data.height) && Number(data.height) >= 32 && Number(data.height) <= 4096
    && (data.image_file_name === null || (text(data.image_file_name, 255) && !/[\/\\]/.test(data.image_file_name)
      && name !== null && !['.', '..'].includes(name) && new TextEncoder().encode(name).length <= 255))
    && (data.background_color == null || (typeof data.background_color === 'string' && /^#[0-9a-f]{6}$/i.test(data.background_color)))
}

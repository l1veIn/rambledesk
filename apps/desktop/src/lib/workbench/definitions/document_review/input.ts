import { object, fields, list, text, optionalText, uniqueId, type RecordValue } from '../validation'

export function validDocumentReviewInput(data: RecordValue): boolean {
  if (!fields(data, ['title', 'source_version', 'paragraphs']) || !text(data.title, 200) || !text(data.source_version, 128) ||
    !list(data.paragraphs, 1, 200)) return false
  const ids = new Set<string>()
  let total = 0
  return data.paragraphs.every((paragraph) => {
    if (!object(paragraph) || !fields(paragraph, ['id', 'label', 'text']) || !uniqueId(paragraph.id, ids) ||
      !text(paragraph.text, 8000) || !optionalText(paragraph.label, 128)) return false
    total += [...paragraph.text].length
    return total <= 120000
  })
}


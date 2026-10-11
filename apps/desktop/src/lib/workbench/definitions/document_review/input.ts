import { fields, text, type RecordValue } from '../validation'

export function validDocumentReviewInput(data: RecordValue): boolean {
  return fields(data, ['title', 'source_version', 'markdown']) && text(data.title, 200)
    && text(data.source_version, 128) && text(data.markdown, 120000)
}

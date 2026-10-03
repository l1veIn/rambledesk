import { fields, list, object, text, uniqueId, type RecordValue } from '../validation'
import { parseReviewDiff } from '../../diff-review/diffModel'

export function validDiffReviewInput(data: RecordValue): boolean {
  if (!fields(data, ['title', 'source_version', 'files']) || !text(data.title, 200) || !text(data.source_version, 128)
    || !list(data.files, 1, 100)) return false
  const ids = new Set<string>()
  let total = 0
  return data.files.every((file) => {
    if (!object(file) || !fields(file, ['id', 'old_path', 'new_path', 'diff']) || !uniqueId(file.id, ids)
      || !text(file.old_path, 2000) || !text(file.new_path, 2000) || !text(file.diff, 120000)) return false
    total += [...file.diff].length
    return total <= 500000 && parseReviewDiff(file.diff) !== null
  })
}

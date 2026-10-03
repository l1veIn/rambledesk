import { fields, list, object, text, uniqueId, type RecordValue } from '../validation'

export function validTableReviewInput(data: RecordValue): boolean {
  if (!fields(data, ['title', 'source_version', 'columns', 'rows']) || !text(data.title, 200)
    || !text(data.source_version, 128) || !list(data.columns, 1, 100) || !list(data.rows, 1, 1000)
    || data.columns.length * data.rows.length > 20000) return false
  const columnCount = data.columns.length
  const columns = new Set<string>(), rows = new Set<string>()
  if (!data.columns.every((column) => object(column) && fields(column, ['id', 'label'])
    && uniqueId(column.id, columns) && text(column.label, 200))) return false
  let total = 0
  return data.rows.every((row) => object(row) && fields(row, ['id', 'cells']) && uniqueId(row.id, rows)
    && list(row.cells, columnCount, columnCount) && row.cells.every((cell) => {
      if (!text(cell, 4000, false)) return false
      total += [...cell].length
      return total <= 500000
    }))
}

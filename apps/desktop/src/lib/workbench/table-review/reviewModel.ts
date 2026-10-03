import type { TableReviewComment, TableReviewData } from '../../generated/feedback'
import type { TableReviewState } from '../definitions/table_review/state'
import { text } from '../definitions/validation'

export type TableCell = { row_id: string; column_id: string }
export const cellIdentity = (cell: TableCell) => `${cell.row_id}:${cell.column_id}`
export const emptyTableReviewState = (): TableReviewState => ({ type: 'table_review', changes: [], comments: [] })
export const sameCell = (a: TableCell, b: TableCell) => a.row_id === b.row_id && a.column_id === b.column_id
export function columnLetter(index: number): string {
  let label = ''
  for (let number = index + 1; number > 0; number = Math.floor((number - 1) / 26)) label = String.fromCharCode(65 + (number - 1) % 26) + label
  return label
}
export function cellPosition(data: TableReviewData, cell: TableCell): { row: number; column: number } | null {
  const row = data.rows.findIndex((item) => item.id === cell.row_id), column = data.columns.findIndex((item) => item.id === cell.column_id)
  return row >= 0 && column >= 0 ? { row, column } : null
}
export function originalCellValue(data: TableReviewData, cell: TableCell): string | null {
  const position = cellPosition(data, cell)
  return position ? data.rows[position.row].cells[position.column] : null
}
export function cellLabel(data: TableReviewData, cell: TableCell): string {
  const position = cellPosition(data, cell)
  return position ? `${columnLetter(position.column)}${position.row + 1} · ${data.columns[position.column].label}` : 'Unavailable cell'
}
export function validateTableReviewState(data: TableReviewData, state: TableReviewState | null): string | null {
  if (!state) return null
  for (const change of state.changes) {
    const original = originalCellValue(data, change)
    if (original === null) return 'A suggestion no longer matches the original table.'
    if (change.value === original) return 'Change the suggested value or remove the unchanged suggestion before submitting.'
  }
  for (const comment of state.comments) {
    if (!cellPosition(data, comment)) return 'A comment no longer matches the original table.'
    if (!text(comment.body, 4000)) return 'Write a comment or remove its empty draft before submitting.'
  }
  return null
}
export function suggestCell(data: TableReviewData, state: TableReviewState, cell: TableCell, value: string,
  keepUnchanged = false): TableReviewState | null {
  const original = originalCellValue(data, cell)
  if (original === null || !text(value, 4000, false)) return null
  const others = state.changes.filter((change) => !sameCell(change, cell))
  if (!keepUnchanged && value === original) return { ...state, changes: others }
  if (others.length >= 20000 || others.reduce((sum, change) => sum + [...change.value].length, [...value].length) > 500000) return null
  return { ...state, changes: [...others, { ...cell, value }] }
}
export function prepareTableComment(data: TableReviewData, state: TableReviewState, cell: TableCell,
  createId: () => string): { state: TableReviewState; comment: TableReviewComment } | null {
  if (!cellPosition(data, cell)) return null
  const existing = state.comments.find((comment) => sameCell(comment, cell))
  if (existing) return { state, comment: existing }
  const id = createId()
  if (state.comments.length >= 500 || !/^[a-z0-9][a-z0-9_-]{0,63}$/.test(id) || state.comments.some((comment) => comment.id === id)) return null
  const comment: TableReviewComment = { id, ...cell, body: '' }
  return { state: { ...state, comments: [...state.comments, comment] }, comment }
}

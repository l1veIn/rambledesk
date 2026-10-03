import { describe, expect, it } from 'vitest'
import { tableReviewDefinition } from '../definitions/table_review/definition'
import { repositoryGuideTable as data } from '../definitions/table_review/examples'
import { readTableReviewState } from '../definitions/table_review/state'
import { emptyTableReviewState, originalCellValue, prepareTableComment, suggestCell, validateTableReviewState, columnLetter } from './reviewModel'

const cell = { row_id: 'ramble', column_id: 'steps' }
const spec = { type: 'table_review', version: 1, data }
describe('table review fixed-source suggestions', () => {
  it('keeps immutable source values while clear and independent cell suggestions survive', () => {
    const snapshot = JSON.stringify(data), empty = emptyTableReviewState()
    const changed = suggestCell(data, empty, cell, '')!
    expect(changed.changes).toEqual([{ ...cell, value: '' }])
    expect(validateTableReviewState(data, changed)).toBeNull()
    const second = suggestCell(data, changed, { ...cell, column_id: 'workbench' }, 'Ramble')!
    expect(second.changes).toHaveLength(2)
    expect(suggestCell(data, second, cell, originalCellValue(data, cell)!)!.changes).toEqual([second.changes[1]])
    expect(JSON.stringify(data)).toBe(snapshot)
    expect(empty.changes).toEqual([])
  })
  it('retains an unfinished original-value voice draft but refuses submitting it', () => {
    const draft = suggestCell(data, emptyTableReviewState(), cell, '5', true)!
    expect(readTableReviewState(draft)).toEqual(draft)
    expect(tableReviewDefinition.hasInput(spec, draft)).toBe(true)
    expect(tableReviewDefinition.complete(spec, draft)).toBe(false)
    expect(tableReviewDefinition.complete(spec, null)).toBe(true)
    expect(tableReviewDefinition.hasInput(spec, null)).toBe(false)
  })
  it('prepares the first voice comment without another cell or duplicate ID, accepting empty drafts only before submit', () => {
    const prepared = prepareTableComment(data, emptyTableReviewState(), cell, () => 'first')!
    expect(prepared.comment).toEqual({ id: 'first', ...cell, body: '' })
    expect(readTableReviewState(prepared.state)).toEqual(prepared.state)
    expect(validateTableReviewState(data, prepared.state)).toContain('empty draft')
    expect(prepareTableComment(data, prepared.state, cell, () => 'ignored')).toEqual(prepared)
    expect(prepareTableComment(data, prepared.state, { ...cell, column_id: 'workbench' }, () => 'first')).toBeNull()
    expect(prepareTableComment(data, prepared.state, { ...cell, row_id: 'missing' }, () => 'new')).toBeNull()
  })
  it('rejects unknown coordinates, duplicate changes, malformed scalar values and total-budget overflows', () => {
    expect(suggestCell(data, emptyTableReviewState(), { ...cell, column_id: 'missing' }, 'x')).toBeNull()
    expect(suggestCell(data, emptyTableReviewState(), cell, '\uD800')).toBeNull()
    expect(suggestCell(data, emptyTableReviewState(), cell, '😀'.repeat(4001))).toBeNull()
    expect(suggestCell(data, emptyTableReviewState(), cell, '😀'.repeat(4000))).not.toBeNull()
    const draft = { ...emptyTableReviewState(), changes: [{ ...cell, value: '6' }, { ...cell, value: '7' }] }
    expect(readTableReviewState(draft)).toBeNull()
    const tooLarge = { ...emptyTableReviewState(), changes: Array.from({ length: 126 }, (_, index) => ({ row_id: `r-${index}`, column_id: 'column', value: 'x'.repeat(4000) })) }
    expect(readTableReviewState(tooLarge)).toBeNull()
    expect(validateTableReviewState(data, { ...emptyTableReviewState(), changes: [{ row_id: 'missing', column_id: 'steps', value: '7' }] })).toContain('original table')
    expect(columnLetter(25)).toBe('Z'); expect(columnLetter(26)).toBe('AA'); expect(columnLetter(99)).toBe('CV')
  })
  it('accepts exact rectangular immutable input and rejects jagged/duplicate/extra-field input', () => {
    expect(tableReviewDefinition.accepts(data)).toBe(true)
    expect(tableReviewDefinition.accepts({ ...data, rows: [{ ...data.rows[0], cells: ['one'] }] })).toBe(false)
    expect(tableReviewDefinition.accepts({ ...data, rows: [data.rows[0], data.rows[0]] })).toBe(false)
    expect(tableReviewDefinition.accepts({ ...data, columns: [data.columns[0], data.columns[0]] })).toBe(false)
    expect(tableReviewDefinition.accepts({ ...data, formulas: [] })).toBe(false)
    expect(tableReviewDefinition.accepts({ ...data, title: '\u0085' })).toBe(false)
  })
})

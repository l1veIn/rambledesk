import { describe, expect, it } from 'vitest'
import { sortDefinition } from './definition'

const spec = sortDefinition.examples![0].spec
const original = { type: 'sort' as const, order: ['quickstart', 'errors', 'completion', 'config'], removed_ids: [], edited_items: [] }

describe('sort workbench contract', () => {
  it('accepts original and reordered complete permutations independently of body feedback', () => {
    expect(sortDefinition.accepts(spec.data as Record<string, unknown>)).toBe(true)
    expect(sortDefinition.complete(spec, original)).toBe(true)
    expect(sortDefinition.hasInput(spec, { ...original, order: [...original.order].reverse() })).toBe(true)
    expect(sortDefinition.submissionMessage!(spec, original)).toBeNull()
    expect(sortDefinition.complete(spec, null)).toBe(false)
    for (const order of [[], ['quickstart'], ['quickstart', 'errors', 'errors', 'config'], ['quickstart', 'errors', 'completion', 'unknown']]) {
      expect(sortDefinition.complete(spec, { ...original, order })).toBe(false)
      expect(sortDefinition.hasInput(spec, { ...original, order })).toBe(false)
    }
  })
  it('rejects unknown keys, malformed states and changed item sets', () => {
    expect(sortDefinition.decodeState({ ...original, future: true })).toBeNull()
    expect(sortDefinition.decodeState({ type: 'sort' })).toBeNull()
    expect(sortDefinition.decodeState({ type: 'sort', order: ['quickstart', 2] })).toBeNull()
    expect(sortDefinition.decodeState({ type: 'sort', order: ['a', 'a'] })).toBeNull()
    expect(sortDefinition.decodeState({ ...original, order: [...original.order].reverse() })).toEqual({ ...original, order: [...original.order].reverse() })
    expect(sortDefinition.accepts({ ...spec.data, extra: 1 })).toBe(false)
    expect(sortDefinition.accepts({ title: 'Priority', items: [{ id: 'a', label: 'A', extra: 1 }, { id: 'b', label: 'B' }] })).toBe(false)
    expect(sortDefinition.complete({ ...spec, data: { title: 'Changed', items: [{ id: 'a', label: 'A' }, { id: 'b', label: 'B' }] } }, original)).toBe(false)
  })
  it('enforces item uniqueness and Unicode scalar limits at both boundaries', () => {
    const items = [{ id: 'a', label: '😀'.repeat(200) }, { id: 'b', label: 'B' }]
    expect(sortDefinition.accepts({ title: '😀'.repeat(200), items })).toBe(true)
    expect(sortDefinition.accepts({ title: '😀'.repeat(201), items })).toBe(false)
    expect(sortDefinition.accepts({ title: 'Priority', items: [{ id: '😀'.repeat(64), label: 'A' }, items[1]] })).toBe(true)
    expect(sortDefinition.accepts({ title: 'Priority', items: [{ id: 'id:dnd-shadow-placeholder-0000', label: 'A' }, items[1]] })).toBe(true)
    for (const invalid of [[], items.slice(0, 1), Array.from({ length: 31 }, (_, i) => ({ id: String(i), label: 'A' })),
      [{ id: 'a', label: 'A' }, { id: 'a', label: 'B' }], [{ id: ' ', label: 'A' }, items[1]],
      [{ id: 'a', label: '😀'.repeat(201) }, items[1]], [{ id: 'a', label: '\uD800' }, items[1]],
      [{ id: 'a\0', label: 'A' }, items[1]]]) {
      expect(sortDefinition.accepts({ title: 'Priority', items: invalid })).toBe(false)
    }
    expect(sortDefinition.decodeState({ type: 'sort', order: ['😀'.repeat(64), 'b'] })).not.toBeNull()
    expect(sortDefinition.decodeState({ type: 'sort', order: ['😀'.repeat(65), 'b'] })).toBeNull()
  })
  it('defaults legacy drafts, validates the kept/deleted partition and retains edits for removed IDs', () => {
    expect(sortDefinition.decodeState({ type: 'sort', order: original.order })).toEqual(original)
    const changed = { ...original, order: ['quickstart', 'completion', 'config'], removed_ids: ['errors'], edited_items: [{ id: 'config', label: '项目配置' }, { id: 'errors', label: '错误诊断' }] }
    expect(sortDefinition.complete(spec, changed)).toBe(true)
    for (const invalid of [{ ...changed, removed_ids: [] }, { ...changed, removed_ids: ['errors', 'config'] },
      { ...changed, removed_ids: ['unknown'] }, { ...changed, edited_items: [{ id: 'unknown', label: 'Unknown' }] }]) {
      expect(sortDefinition.complete(spec, invalid)).toBe(false)
    }
    for (const edited_items of [[{ id: 'config', label: 'A' }, { id: 'config', label: 'B' }],
      [{ id: 'config', label: 'A', extra: true }], [{ id: 'config', label: '😀'.repeat(201) }], [{ id: 'config', label: '\0' }]]) {
      expect(sortDefinition.decodeState({ ...original, edited_items })).toBeNull()
    }
    expect(sortDefinition.decodeState({ ...original, removed_ids: null })).toBeNull()
    expect(sortDefinition.decodeState({ ...original, edited_items: null })).toBeNull()
  })
  it('saves unfinished label edits but blocks publication until retained labels are visible', () => {
    const blank = { ...original, edited_items: [{ id: 'config', label: '' }] }
    expect(sortDefinition.decodeState(blank)).toEqual(blank)
    expect(sortDefinition.hasInput(spec, blank)).toBe(true)
    expect(sortDefinition.complete(spec, blank)).toBe(false)
    expect(sortDefinition.submissionMessage!(spec, blank)).toContain('名称')
    expect(sortDefinition.complete(spec, { ...blank, edited_items: [{ id: 'config', label: '😀'.repeat(200) }] })).toBe(true)
    expect(sortDefinition.complete(spec, { ...blank, order: original.order.filter((id) => id !== 'config'), removed_ids: ['config'] })).toBe(true)
    expect(sortDefinition.complete(spec, { ...blank, order: [], removed_ids: original.order })).toBe(true)
    expect(sortDefinition.complete(spec, { ...blank, edited_items: [{ id: 'config', label: ' \n\t' }] })).toBe(false)
  })
})

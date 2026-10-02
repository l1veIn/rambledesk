import type { SortData, SortState, WorkbenchSpec, WorkbenchState } from '../../../generated/feedback'
import type { WorkbenchDefinition } from '../contracts'
import { record } from '../stateShape'
import { fields as keys, list, text } from '../validation'

export function sortData(value: unknown): value is SortData {
  if (!record(value) || !keys(value, ['title', 'items']) || !text(value.title, 200) || !list(value.items, 2, 30)) return false
  const ids = new Set<string>()
  return value.items.every((item) => {
    if (!record(item) || !keys(item, ['id', 'label']) || !text(item.id, 64) || !text(item.label, 200) || ids.has(item.id)) return false
    ids.add(item.id)
    return true
  })
}
export function sortState(value: unknown): WorkbenchState | null {
  if (!record(value) || !keys(value, ['type', 'order', 'removed_ids', 'edited_items']) || value.type !== 'sort') return null
  const removed = value.removed_ids === undefined ? [] : value.removed_ids
  const edits = value.edited_items === undefined ? [] : value.edited_items
  if (!list(value.order, 0, 30) || !list(removed, 0, 30) || !list(edits, 0, 30)
    || !value.order.every((id) => text(id, 64)) || !removed.every((id) => text(id, 64))
    || value.order.length + removed.length > 30 || new Set([...value.order, ...removed]).size !== value.order.length + removed.length) return null
  const editedIds = new Set<string>()
  const normalizedEdits: SortState['edited_items'] = []
  if (!edits.every((item) => {
    if (!record(item) || !keys(item, ['id', 'label']) || !text(item.id, 64) || !text(item.label, 200, false) || editedIds.has(item.id)) return false
    editedIds.add(item.id)
    normalizedEdits.push({ id: item.id, label: item.label })
    return true
  })) return null
  return { type: 'sort', order: [...value.order], removed_ids: [...removed], edited_items: normalizedEdits }
}
export function validSortOrder(data: SortData, order: readonly string[], removed: readonly string[] = []): boolean {
  const ids = new Set(data.items.map((item) => item.id))
  const selected = [...order, ...removed]
  return selected.length === ids.size && new Set(selected).size === selected.length && selected.every((id) => ids.has(id))
}
export function validSortDraft(data: SortData, state: SortState): boolean {
  return validSortOrder(data, state.order, state.removed_ids) && state.edited_items.every((item) => data.items.some((source) => source.id === item.id))
}
export function sortLabel(data: SortData, state: SortState, id: string): string {
  return state.edited_items.find((item) => item.id === id)?.label ?? data.items.find((item) => item.id === id)?.label ?? ''
}
const hasInput: WorkbenchDefinition['hasInput'] = (spec, value) => {
  const state = sortState(value)
  return spec.type === 'sort' && spec.version === 1 && sortData(spec.data)
    && state?.type === 'sort' && validSortDraft(spec.data, state)
}
const complete: WorkbenchDefinition['complete'] = (spec, value) => {
  const state = sortState(value)
  const data = spec.data
  return hasInput(spec, state) && state?.type === 'sort' && sortData(data)
    && state.order.every((id) => text(sortLabel(data, state, id), 200))
}

export const sortDefinition: WorkbenchDefinition = {
  type: 'sort', version: 1,
  accepts: sortData,
  decodeState: sortState,
  hasInput,
  complete,
  submissionMessage: (spec, state) => complete(spec, state) ? null : hasInput(spec, state) ? '请填写保留选项的名称，或删除空白选项后提交。' : '请完成列表排序后提交。',
  layout: { padded: true, interactivePreview: false, expanded: false },
  loadView: () => import('./View.svelte'),
  examples: [{ order: 7, title: '7/7 · 拖动排序', markdown: '按你希望优先改进的顺序排列 CLI 功能。拖动手柄调整顺序，也可以编辑名称、删除不需要的选项。删除后可恢复；保留当前顺序也可以提交。',
    spec: { type: 'sort', version: 1, data: { title: 'CLI 功能优先级', items: [
      { id: 'quickstart', label: '快速上手' }, { id: 'errors', label: '清晰的错误提示' },
      { id: 'completion', label: '命令补全' }, { id: 'config', label: '配置文件' },
    ] } satisfies SortData } satisfies WorkbenchSpec,
    attachments: [{ name: '排序体验.md', content: '# 排序体验\n\n1. 拖动左侧手柄，将功能按优先级从上到下排列。\n2. 键盘聚焦手柄，按空格或回车开始，方向键移动，再按空格或回车完成。\n3. 点击编辑按钮修改选项名称；清空名称会保存草稿，但需填写名称或删除该选项后才能提交。\n4. 删除选项，再从已删除列表恢复，检查编辑后的名称仍然保留。\n5. 切换请求再返回，检查草稿。排序工作台只在普通视图中使用，无全屏入口。\n6. 可在反馈正文补充理由；提交后检查只读排序。' }] }],
}

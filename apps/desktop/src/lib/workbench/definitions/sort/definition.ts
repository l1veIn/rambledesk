import type { SortData, WorkbenchSpec, WorkbenchState } from '../../../generated/feedback'
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
  if (!record(value) || !keys(value, ['type', 'order']) || value.type !== 'sort' || !list(value.order, 2, 30)
    || !value.order.every((id) => text(id, 64)) || new Set(value.order).size !== value.order.length) return null
  return { type: 'sort', order: [...value.order] }
}
export function validSortOrder(data: SortData, order: readonly string[]): boolean {
  const ids = new Set(data.items.map((item) => item.id))
  return order.length === ids.size && new Set(order).size === order.length && order.every((id) => ids.has(id))
}
const complete: WorkbenchDefinition['complete'] = (spec, state) => spec.type === 'sort' && spec.version === 1
  && sortData(spec.data) && state?.type === 'sort' && !!sortState(state) && validSortOrder(spec.data, state.order)

export const sortDefinition: WorkbenchDefinition = {
  type: 'sort', version: 1,
  accepts: sortData,
  decodeState: sortState,
  hasInput: complete,
  complete,
  submissionMessage: (spec, state) => complete(spec, state) ? null : '请完成列表排序后提交。',
  layout: { padded: true, interactivePreview: false, expanded: true },
  loadView: () => import('./View.svelte'),
  examples: [{ order: 7, title: '7/7 · 拖动排序', markdown: '按你希望优先改进的顺序排列 CLI 功能。拖动条目左侧的手柄，也可使用上下移动按钮；保留当前顺序也可以提交。',
    spec: { type: 'sort', version: 1, data: { title: 'CLI 功能优先级', items: [
      { id: 'quickstart', label: '快速上手' }, { id: 'errors', label: '清晰的错误提示' },
      { id: 'completion', label: '命令补全' }, { id: 'config', label: '配置文件' },
    ] } satisfies SortData } satisfies WorkbenchSpec,
    attachments: [{ name: '排序体验.md', content: '# 排序体验\n\n1. 拖动左侧手柄，将功能按优先级从上到下排列。\n2. 用上下移动按钮或手柄的键盘操作调整顺序。\n3. 切换请求，再返回，检查草稿顺序。\n4. 打开全屏工作台，再返回，检查顺序。\n5. 可在反馈正文补充理由；提交后检查只读排序。' }] }],
}

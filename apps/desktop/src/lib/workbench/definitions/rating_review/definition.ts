import type { RatingReviewData, WorkbenchSpec, WorkbenchState } from '../../../generated/feedback'
import type { WorkbenchDefinition } from '../contracts'
import type { WorkbenchFieldAdapter } from '../../fields/contracts'
import { fieldFingerprint, validFieldText } from '../../fields/value'
import { record } from '../stateShape'
import { fields as keys } from '../validation'

const validScore = (value: unknown): value is number => Number.isInteger(value) && Number(value) >= 1 && Number(value) <= 5
export function ratingMaterialVersion(data: unknown): string {
  const text = fieldFingerprint(data)
  let a = 2166136261, b = 5381
  for (let i = 0; i < text.length; i += 1) { a = Math.imul(a ^ text.charCodeAt(i), 16777619); b = Math.imul(b, 33) ^ text.charCodeAt(i) }
  return `${text.length}-${a >>> 0}-${b >>> 0}`
}
export function ratingReviewState(value: unknown): WorkbenchState | null {
  if (!record(value) || !keys(value, ['type', 'score', 'note']) || value.type !== 'rating_review' || !(value.score == null || validScore(value.score)) || !validFieldText(value.note, 4000)) return null
  return { type: 'rating_review', score: value.score ?? null, note: value.note }
}
export const opinionField: WorkbenchFieldAdapter = {
  accepts: (target) => target.destination.kind === 'workbench_field' && target.destination.workbenchType === 'rating_review'
    && target.destination.version === 1 && target.destination.field === 'opinion',
  read: ({ spec, state, target }) => {
    const destination = target.destination
    if (spec.type !== 'rating_review' || spec.version !== 1 || state.type !== 'rating_review'
      || destination.kind !== 'workbench_field' || destination.entityId !== 'review'
      || destination.sourceVersion !== ratingMaterialVersion(spec.data)) throw new Error('The review material has changed. Select the opinion field again.')
    return { value: state.note, limit: 4000, contract: fieldFingerprint(spec.data), identity: 'review/opinion',
      replace: (note) => ({ ...state, note }) }
  },
  text: (state, target) => state.type === 'rating_review' && target.destination.kind === 'workbench_field'
    && target.destination.entityId === 'review' ? state.note : null,
  removeAttachment: (state, _id, removeText) => state.type === 'rating_review' ? { ...state, note: removeText(state.note) } : state,
}
const complete: WorkbenchDefinition['complete'] = (_, state) => state?.type === 'rating_review' && validScore(state.score) && validFieldText(state.note, 4000)
export const ratingReviewDefinition: WorkbenchDefinition = {
  type: 'rating_review', version: 1,
  accepts: (data) => keys(data, ['title', 'material']) && validFieldText(data.title, 200) && !!data.title.trim()
    && validFieldText(data.material, 120000) && !!data.material.trim(),
  decodeState: ratingReviewState,
  hasInput: (_, state) => state?.type === 'rating_review' && (validScore(state.score) || !!state.note.trim()),
  complete, submissionMessage: (spec, state) => complete(spec, state) ? null : '请选择 1 至 5 分后提交。',
  layout: { padded: true, interactivePreview: false, expanded: true }, fields: [opinionField],
  loadView: () => import('./View.svelte'),
  examples: [{ order: 6, title: '评分＋意见 · 扩展演练', markdown: '阅读材料，选择评分并留下意见。体验草稿恢复、语音和附件，以及全屏页签。',
    spec: { type: 'rating_review', version: 1, data: { title: '评审体验提案', material: '让开发者通过工作台收集结构化判断，同时保留用户自由表达的反馈。请评估这个提案是否清楚、可用。' } satisfies RatingReviewData } satisfies WorkbenchSpec,
    attachments: [{ name: '评分体验.md', content: '# 评分体验\n\n1. 先写意见但不选评分，确认不能提交。\n2. 选择评分，切换请求再返回，检查草稿。\n3. 在意见框输入文字、语音或附件。\n4. 切换全屏页签，再返回。\n5. 提交后检查只读历史。' }] }],
}

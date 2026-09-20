import type { WorkbenchSpec } from '$lib/generated/feedback'
import type { FeedbackWorkspaceView } from '$lib/feedback'
import { previewFixtures } from '$lib/preview/previewFixtures'

export const workbenchExamples: WorkbenchSpec[] = [
  { type: 'ramble', version: 1, data: { actions: [
    { id: 'try', instruction: '体验右侧反馈编辑器，记录你的感受。' },
  ] } },
  { type: 'questions', version: 1, data: { questions: [
    { id: 'audience', label: '用户', prompt: '这个产品最先服务哪一类用户？', allowOther: true, options: [{ value: 'individuals', label: '独立开发者', description: '先把单人的反馈与决策流程做好。' }, { value: 'teams', label: '小团队', description: '优先考虑多人协作和共同评审。' }] },
    { id: 'priority', label: '重点', prompt: '第一版最重要的能力是什么？', allowOther: true, options: [{ value: 'feedback', label: '快速反馈', description: '用最少操作把体验和意见交给 Agent。' }, { value: 'decisions', label: '结构化决策', description: '把问答、方案选择做得清楚可追踪。' }] },
    { id: 'concern', label: '顾虑', prompt: '你最担心什么问题？', allowOther: true, options: [{ value: 'complexity', label: '操作太复杂', description: '希望打开之后能立即理解怎么操作。' }, { value: 'loss', label: '答案丢失或关联错误', description: '切换页面、重新打开后仍应保留准确的回答。' }] },
  ] } },
  { type: 'single_choice', version: 1, data: {
    prompt: '你更倾向哪一种布局？', options: [
      { id: 'compact', label: '紧凑布局：同屏展示更多信息' },
      { id: 'spacious', label: '宽松布局：留出更多阅读和输入空间' },
      { id: 'adaptive', label: '自适应布局：根据内容和窗口自动调整' },
    ],
  } },
]

export function workbenchPreviewWorkspace(index: number): FeedbackWorkspaceView {
  const workspace = structuredClone(previewFixtures.workspace)
  const spec = workbenchExamples[index]
  workspace.workbench = spec
  workspace.request = { ...workspace.request, request_id: `workbench-preview-${index}`, title: ['自由反馈', '逐项问答', '方案单选'][index], what_happened: '体验三种反馈形式，看看哪种更适合当前任务。', status: 'in_progress', resolution: null, allow_finish: false, final_summary: null }
  workspace.actions = 'actions' in spec.data ? spec.data.actions : 'questions' in spec.data
    ? spec.data.questions.map((item) => ({ id: item.id, instruction: item.prompt }))
    : spec.data.options.map((item) => ({ id: item.id, instruction: item.label }))
  workspace.request_attachments = []
  workspace.attachments = []
  workspace.draft = { document_json: null, body_markdown: '', saved_revision: 0, updated_at: null }
  workspace.feedback = null
  return workspace
}

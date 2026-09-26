import type { WorkbenchSpec, RambleData, QuestionsData, SingleChoiceData } from '$lib/generated/feedback'
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
  { type: 'document_review', version: 1, data: {
    title: '把灵感留在现场', source_version: '视频脚本 · v0.3 · 约 90 秒',
    paragraphs: [
      { id: 'opening', label: '开场 · 00:00–00:12', text: '你有没有过这样的时刻：刚试完一个新功能，脑子里有很多想法，等打开聊天窗口，却只记得一句——“感觉还可以”。' },
      { id: 'problem', label: '问题 · 00:12–00:28', text: '体验发生在产品里，反馈却留在另一个地方。你切换窗口、找截图、回忆刚才的操作。真正重要的那个瞬间，就这样被打断了。' },
      { id: 'demo', label: '演示 · 00:28–00:52', text: '现在，试着一边操作，一边说出你的感受。哪里卡住了，截一张图；哪里不符合预期，直接记下来。RambleDesk 会自动理解你的所有意图，替你做出最好的决定。' },
      { id: 'value', label: '价值 · 00:52–01:12', text: '文字、语音和截图，最终汇成一份清楚的反馈。你可以回看、补充、修改，确认之后再交给 Agent。你的判断始终属于你，工具只负责把它完整地带回去。' },
      { id: 'closing', label: '结尾 · 01:12–01:30', text: '下一次灵感出现时，不必急着把它整理得完美。先留下真实的体验，再让好的改变从这里开始。' },
    ],
  } },
]

export const workbenchPreviewLabels = ['Ramble 自由反馈', '逐项问答', '方案单选', '文稿审阅']

export function workbenchPreviewWorkspace(index: number): FeedbackWorkspaceView {
  const workspace = structuredClone(previewFixtures.workspace)
  const spec = workbenchExamples[index]
  workspace.workbench = spec
  workspace.request = { ...workspace.request, request_id: `workbench-preview-${index}`, title: workbenchPreviewLabels[index], what_happened: index === 3 ? '请审阅这份视频脚本，重点检查产品表述是否准确、口播是否自然，并留下修改建议。' : '体验不同反馈形式，看看哪种更适合当前任务。', status: 'in_progress', resolution: null, allow_finish: false, final_summary: null }
  workspace.actions = spec.type === 'ramble' ? (spec.data as RambleData).actions : spec.type === 'questions'
    ? (spec.data as QuestionsData).questions.map((item) => ({ id: item.id, instruction: item.prompt }))
    : spec.type === 'single_choice' ? (spec.data as SingleChoiceData).options.map((item) => ({ id: item.id, instruction: item.label })) : []
  workspace.request_attachments = []
  workspace.attachments = []
  workspace.draft = { document_json: null, body_markdown: '', saved_revision: 0, updated_at: null }
  workspace.feedback = null
  return workspace
}

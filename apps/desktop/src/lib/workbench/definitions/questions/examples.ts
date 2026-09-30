import type { QuestionsData } from '../../../generated/feedback'
import type { WorkbenchExample } from '../contracts'

export const examples: readonly WorkbenchExample[] = [
  { order: 1, title: '逐项问答', markdown: '体验工作台，在右侧记录反馈后提交。', spec: { type: 'questions', version: 1, data: { questions: [
    { id: 'audience', label: '用户', prompt: '这个产品最先服务哪一类用户？', allowOther: true, options: [{ value: 'individuals', label: '独立开发者', description: '先把单人的反馈与决策流程做好。' }, { value: 'teams', label: '小团队', description: '优先考虑多人协作和共同评审。' }] },
    { id: 'priority', label: '重点', prompt: '第一版最重要的能力是什么？', allowOther: true, options: [{ value: 'feedback', label: '快速反馈', description: '用最少操作把体验和意见交给 Agent。' }, { value: 'decisions', label: '结构化决策', description: '把问答、方案选择做得清楚可追踪。' }] },
    { id: 'concern', label: '顾虑', prompt: '你最担心什么问题？', allowOther: true, options: [{ value: 'complexity', label: '操作太复杂', description: '希望打开之后能立即理解怎么操作。' }, { value: 'loss', label: '答案丢失或关联错误', description: '切换页面、重新打开后仍应保留准确的回答。' }] },
  ] } } },
  { order: 2, title: '逐项问答 · 单题选择', markdown: '体验工作台，在右侧记录反馈后提交。', spec: { type: 'questions', version: 1, data: { questions: [{
    id: 'layout', label: '布局', prompt: '你更倾向哪一种布局？', allowOther: false, options: [
      { value: 'compact', label: '紧凑布局：同屏展示更多信息' },
      { value: 'spacious', label: '宽松布局：留出更多阅读和输入空间' },
      { value: 'adaptive', label: '自适应布局：根据内容和窗口自动调整' },
    ],
  }] } } },
].map((example) => ({...example,actions:(example.spec.data as QuestionsData).questions.map((item) => ({id:item.id,instruction:item.prompt}))}))

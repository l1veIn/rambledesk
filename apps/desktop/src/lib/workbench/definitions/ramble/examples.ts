import type { RambleData } from '../../../generated/feedback'
import type { WorkbenchExample } from '../contracts'

export const examples: readonly WorkbenchExample[] = [
  { order: 0, title: 'Ramble 自由反馈', markdown: '体验工作台，在右侧记录反馈后提交。', spec: { type: 'ramble', version: 1, data: { actions: [
    { id: 'try', instruction: '体验右侧反馈编辑器，记录你的感受。' },
  ] } } },
].map((example) => ({...example,actions:(example.spec.data as RambleData).actions}))

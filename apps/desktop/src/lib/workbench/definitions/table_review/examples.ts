import type { TableReviewData } from '../../../generated/feedback'
import type { WorkbenchExample } from '../contracts'

// A repository snapshot for development previews, never injected into a user's request.
export const repositoryGuideTable: TableReviewData = {
  title: '工作台引导配置清单', source_version: 'repository-guides-2026-10-03',
  columns: [
    { id: 'workbench', label: '工作台类型' }, { id: 'guide_version', label: '引导版本' },
    { id: 'steps', label: '引导步骤数' }, { id: 'definition', label: '配置文件' },
  ],
  rows: ['ramble', 'document_review', 'web_review', 'terminal', 'visual_feedback', 'diff_review'].map((type) => ({
    id: type, cells: [type, type === 'diff_review' ? '2' : '1', '5', `apps/desktop/src/lib/workbench/definitions/${type}/definition.ts`],
  })),
}
export const examples: readonly WorkbenchExample[] = [{
  order: 9, title: '表格审阅', markdown: '审阅仓库中六个工作台的引导配置快照。选择单元格提出改值建议或批注，原值保持固定；整体意见写在反馈正文。此清单仅用于开发预览。',
  spec: { type: 'table_review', version: 1, data: repositoryGuideTable },
}]

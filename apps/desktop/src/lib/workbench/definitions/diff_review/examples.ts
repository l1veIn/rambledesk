import type { DiffReviewData } from '../../../generated/feedback'
import type { WorkbenchExample } from '../contracts'

export const examples: readonly WorkbenchExample[] = [{
  order: 8, title: '差异评审', markdown: '审阅多文件改动，针对旧版或新版的行、范围和整个分块批注；整体意见写在右侧反馈正文。',
  spec: { type: 'diff_review', version: 1, data: {
    title: '检查错误提示与快速上手说明', source_version: 'cli-review-v1', files: [
      { id: 'errors', old_path: 'src/cli/errors.ts', new_path: 'src/cli/errors.ts', diff: '--- a/src/cli/errors.ts\n+++ b/src/cli/errors.ts\n@@ -1,3 +1,4 @@\n export function missingConfig(path: string) {\n-  return "Missing config";\n+  return `Configuration not found: ${path}`;\n+  // Include the searched path to make the next action clear.\n }\n' },
      { id: 'quickstart', old_path: '/dev/null', new_path: 'docs/quickstart.md', diff: '--- /dev/null\n+++ b/docs/quickstart.md\n@@ -0,0 +1,3 @@\n+# Quick start\n+\n+Run `ramble init`, then `ramble start`.\n' },
    ],
  } satisfies DiffReviewData },
}]

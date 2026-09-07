import { t } from '$lib/i18n'
import type { Locale } from '$lib/preferences'

const chinese: Record<string, string> = {
  Projects: '项目',
  'No project': '未关联项目',
  'External sessions': '外部会话',
  'No project sessions yet': '还没有项目会话',
  'New session in {project}': '在 {project} 中新建会话',
  'Search requests': '搜索请求',
  'Search sessions and projects': '搜索会话和项目',
  'Search by session title, project name, or folder path.': '按会话标题、项目名称或文件夹路径搜索。',
  'No matching sessions': '没有匹配的会话',
  'Clear search': '清除搜索',
  'Show results': '查看结果',
}

export function sessionRailText(locale: Locale, source: string, values: Record<string, string | number> = {}): string {
  let result = locale === 'zh-CN' ? chinese[source] ?? t(locale, source, values) : t(locale, source, values)
  for (const [key, value] of Object.entries(values)) result = result.replaceAll(`{${key}}`, String(value))
  return result
}

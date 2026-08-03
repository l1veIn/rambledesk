import { createOpenAI } from '@ai-sdk/openai'
import { generateText } from 'ai'
import { fetch as tauriFetch } from '@tauri-apps/plugin-http'

import type { ActionInput } from './feedback'
import { t } from './i18n'
import type { CookingProvider, CookingReasoningEffort, Locale } from './preferences'

export type CookingConfig = {
  provider: CookingProvider
  apiKey: string
  baseUrl: string
  model: string
  reasoningEffort: CookingReasoningEffort
  locale: Locale
}

export type CookFeedbackInput = {
  title: string
  whatHappened: string
  actions: ActionInput[]
  uncookedMarkdown: string
}

export async function cookFeedback(
  input: CookFeedbackInput,
  config: CookingConfig,
): Promise<{ markdown: string; model: string }> {
  const apiKey = config.apiKey.trim()
  const modelId = config.model.trim()
  if (!apiKey) throw new Error(t(config.locale, 'Cooking 已开启，但尚未配置 API Key。'))
  if (!modelId) throw new Error(t(config.locale, 'Cooking 已开启，但尚未配置模型名称。'))

  const provider = createOpenAI({
    apiKey,
    baseURL: normalizedBaseUrl(config.provider, config.baseUrl),
    fetch: '__TAURI_INTERNALS__' in window ? tauriFetch : globalThis.fetch,
  })
  const result = await generateText({
    model: provider.chat(modelId),
    temperature: 0.2,
    providerOptions: {
      openai: { reasoningEffort: config.reasoningEffort },
    },
    system: `你是 RambleDesk 的反馈文档编辑器。你的任务是把人类口述或随手记录的 uncooked feedback 整理成准确、正式、可执行的 Markdown 反馈。

必须遵守：
1. 保留原始事实、判断、不确定性和第一人称体验，不得编造测试结果或技术细节。
2. 删除口水词、重复、自我修正和无意义停顿，修复明显的语音转录断句。
3. 合并前后重复内容，但不得淡化问题、负面反馈或用户明确要求。
4. 使用清晰的小标题、段落和列表；只输出最终 Markdown，不要解释编辑过程。
5. 原样保留所有 attachment://<id> 图片引用及其附近语义，不得修改 ID、丢弃图片或凭空新增附件。
6. 不要复述任务简报；正文应聚焦 Operator Feedback。`,
    prompt: `# 请求标题\n${input.title}\n\n# 任务背景\n${input.whatHappened}\n\n# 验收动作\n${input.actions
      .map((action) => `- ${action.id}: ${action.instruction}`)
      .join('\n')}\n\n# Uncooked Operator Feedback\n\n${input.uncookedMarkdown}`,
  })
  const markdown = result.text.trim()
  if (!markdown) {
    throw new Error(t(config.locale, 'Cooking 模型返回了空内容，请检查模型配置后重试。'))
  }
  return {
    markdown,
    model: `${config.provider}/${modelId}`,
  }
}

function normalizedBaseUrl(provider: CookingProvider, configured: string): string | undefined {
  const value = configured.trim().replace(/\/$/, '')
  if (value) return value
  if (provider === 'deepseek') return 'https://api.deepseek.com/v1'
  if (provider === 'openai') return 'https://api.openai.com/v1'
  return undefined
}

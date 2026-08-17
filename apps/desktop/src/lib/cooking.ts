import { createOpenAI } from '@ai-sdk/openai'
import { generateText } from 'ai'
import { fetch as tauriFetch } from '@tauri-apps/plugin-http'

import type { ActionInput } from './feedback'
import { t } from './i18n'
import type { CookingProvider, CookingReasoningEffort, Locale } from './preferences'

export const DEFAULT_COOKING_SYSTEM_PROMPT = `You are the RambleDesk feedback editor. Turn spoken or informal uncooked feedback into accurate, formal, actionable Markdown.

Rules:
1. Keep original facts, judgments, uncertainty, and first-person experience. Do not invent test results or technical details.
2. Remove filler, repetition, self-corrections, and meaningless pauses. Fix obvious speech-to-text breaks.
3. Merge repeated points without softening problems, negative feedback, or explicit requests.
4. Use clear headings, paragraphs, and lists. Output only the final Markdown. Do not explain the edit.
5. Keep every Markdown image and attachment://<id> reference verbatim, including \`![...](attachment://...)\`. Do not change IDs, drop images, replace them with descriptions, or invent attachments.
6. Do not restate the task brief. The body should focus on Operator Feedback.`

export function resolveCookingSystemPrompt(custom: string | null | undefined): string {
  const trimmed = custom?.trim() ?? ''
  return trimmed || DEFAULT_COOKING_SYSTEM_PROMPT
}

export type CookingConfig = {
  provider: CookingProvider
  apiKey: string
  baseUrl: string
  model: string
  reasoningEffort: CookingReasoningEffort
  locale: Locale
  systemPrompt?: string
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
  if (!apiKey) throw new Error(t(config.locale, 'Cooking is enabled, but no API key has been configured.'))
  if (!modelId) throw new Error(t(config.locale, 'Cooking is enabled, but no model name has been configured.'))

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
    system: resolveCookingSystemPrompt(config.systemPrompt),
    prompt: `# 请求标题\n${input.title}\n\n# 任务背景\n${input.whatHappened}\n\n# 验收动作\n${input.actions
      .map((action) => `- ${action.id}: ${action.instruction}`)
      .join('\n')}\n\n# Uncooked Operator Feedback\n\n${input.uncookedMarkdown}`,
  })
  const markdown = result.text.trim()
  if (!markdown) {
    throw new Error(t(config.locale, 'The Cooking model returned an empty response. Check the model configuration and try again.'))
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

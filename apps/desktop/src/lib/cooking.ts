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
5. Keep every Markdown image, attachment://<id> reference, and rambledesk-capture:// marker link verbatim, including \`![...](attachment://...)\` and \`[](rambledesk-capture://...)\`. Do not change IDs, drop images, replace them with descriptions, or invent attachments.
6. Do not restate the task brief. The body should focus on Operator Feedback.
7. Preserve the operator's language. If the uncooked feedback is in Chinese, write the cooked Markdown in Chinese. If it is in English, write it in English. Do not translate the body into another language.`

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

export type LlmFeature = 'Cooking' | 'Light cleanup'

export type ModelTextRequest = {
  config: CookingConfig
  system: string
  prompt: string
}

export type ModelTextGenerator = (
  input: ModelTextRequest,
) => Promise<{ text: string; model: string }>

export function assertLlmReady(config: CookingConfig, feature: LlmFeature): void {
  if (!config.apiKey.trim()) {
    throw new Error(
      t(config.locale, `${feature} is enabled, but no API key has been configured.`),
    )
  }
  if (!config.model.trim()) {
    throw new Error(
      t(config.locale, `${feature} is enabled, but no model name has been configured.`),
    )
  }
}

export async function generateModelText(input: ModelTextRequest): Promise<{ text: string; model: string }> {
  const modelId = input.config.model.trim()
  const provider = createOpenAI({
    apiKey: input.config.apiKey.trim(),
    baseURL: normalizedBaseUrl(input.config.provider, input.config.baseUrl),
    fetch: '__TAURI_INTERNALS__' in window ? tauriFetch : globalThis.fetch,
  })
  const result = await generateText({
    model: provider.chat(modelId),
    temperature: 0.2,
    providerOptions: {
      openai: { reasoningEffort: input.config.reasoningEffort },
    },
    system: input.system,
    prompt: input.prompt,
  })
  return {
    text: result.text.trim(),
    model: `${input.config.provider}/${modelId}`,
  }
}

export async function cookFeedback(
  input: CookFeedbackInput,
  config: CookingConfig,
): Promise<{ markdown: string; model: string }> {
  assertLlmReady(config, 'Cooking')
  const result = await generateModelText({
    config,
    system: resolveCookingSystemPrompt(config.systemPrompt),
    prompt: `# 请求标题\n${input.title}\n\n# 任务背景\n${input.whatHappened}\n\n# 验收动作\n${input.actions
      .map((action) => `- ${action.id}: ${action.instruction}`)
      .join('\n')}\n\n# Uncooked Operator Feedback\n\n${input.uncookedMarkdown}`,
  })
  if (!result.text) {
    throw new Error(t(config.locale, 'The Cooking model returned an empty response. Check the model configuration and try again.'))
  }
  return {
    markdown: result.text,
    model: result.model,
  }
}

function normalizedBaseUrl(provider: CookingProvider, configured: string): string | undefined {
  const value = configured.trim().replace(/\/$/, '')
  if (value) return value
  if (provider === 'deepseek') return 'https://api.deepseek.com/v1'
  if (provider === 'openai') return 'https://api.openai.com/v1'
  return undefined
}

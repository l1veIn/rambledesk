import {
  assertLlmReady,
  generateModelText,
  type CookingConfig,
  type ModelTextGenerator,
} from './cooking'

export type LightCleanupConfig = CookingConfig

export const DEFAULT_LIGHT_CLEANUP_SYSTEM_PROMPT = `You lightly tidy a speech-to-text transcript. This is not Cooking.

Rules:
1. Remove filler and hesitation only: 啊, 嗯, 呃, 那个, um, uh, like (when it is a hesitation), you know. Remove 比如说 only when it is spoken hesitation, not a real "for example".
2. Fix punctuation and sentence breaks introduced by speech-to-text.
3. Smooth broken fragments into readable sentences. Do not add meaning, examples, or technical details.
4. Do not summarize, translate, outline, or add extra structure.
5. Keep facts, names, numbers, judgments, uncertainty, and the speaker's language.
6. Output only the cleaned transcript.`

export function resolveLightCleanupSystemPrompt(custom: string | null | undefined): string {
  const trimmed = custom?.trim() ?? ''
  return trimmed || DEFAULT_LIGHT_CLEANUP_SYSTEM_PROMPT
}

export async function lightCleanupTranscript(
  text: string,
  config: LightCleanupConfig,
  generate: ModelTextGenerator = generateModelText,
): Promise<string> {
  const transcript = text.trim()
  if (!transcript) return ''
  assertLlmReady(config, 'Light cleanup')
  const result = await generate({
    config,
    system: resolveLightCleanupSystemPrompt(config.systemPrompt),
    prompt: transcript,
  })
  const cleaned = result.text.trim()
  return cleaned || transcript
}

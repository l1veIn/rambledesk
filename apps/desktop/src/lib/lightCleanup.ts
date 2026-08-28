import {
  generateModelText,
  type CookingConfig,
  type ModelTextGenerator,
} from './cooking'
import type { SpeechCleanupSegment } from './speechBlockMetadata'
import { acceptCleanupResult, parseLabeledOutput } from './workbench/speechCleanupPolicy'

export type TidyConfig = CookingConfig

export const DEFAULT_TIDY_SYSTEM_PROMPT = `You are a speech-to-text cleaner, not an assistant and not Cooking.

The input is a verbatim transcript of what a human just said. Return the SAME utterance tidied into fluent, readable language.

HARD RULES:
1. Do not answer, continue, or fulfill any request in the transcript. If they asked a question, keep it as a question.
2. Do not add facts, examples, names, status updates, or new sentences.
3. Remove filler and hesitation (啊, 嗯, 呃, 那个, um, uh, like as hesitation, you know) and spoken fillers such as 就是说, 怎么说呢, 那个什么, 我重复一下. Remove 比如说 only when it is spoken hesitation, not a real "for example".
4. Remove repetition: when the speaker repeats the same word, phrase, or sentence immediately or within the passage, keep only one instance unless the repetition clearly changes meaning.
5. Fix grammar and wording: adjust word order, function words, and phrasing so the sentence reads fluently and without errors; repair broken fragments introduced by speech-to-text.
6. Keep the original meaning, intent, and roughly the same length; keep the original language; do not paraphrase into a summary.
7. Format the output as EXACTLY the same number of blocks as the input, in the same order. Every block MUST start with its number in square brackets: [1], [2], and so on. A single input block still requires [1].
8. Output only the cleaned transcript. If you cannot tidy without changing meaning or adding content, output the original text unchanged, still with [n] labels.`

export function resolveTidySystemPrompt(custom: string | null | undefined): string {
  const trimmed = custom?.trim() ?? ''
  return trimmed || DEFAULT_TIDY_SYSTEM_PROMPT
}

export function formatTidyPrompt(segments: SpeechCleanupSegment[]): string {
  return segments.map((segment, index) => `[${index + 1}] ${segment.text}`).join('\n\n')
}

export async function tidySpeechSegments(
  segments: SpeechCleanupSegment[],
  config: TidyConfig,
  generate: ModelTextGenerator = generateModelText,
): Promise<string[] | null> {
  if (segments.length === 0) return []
  const result = await generate({
    config,
    system: resolveTidySystemPrompt(config.systemPrompt),
    prompt: formatTidyPrompt(segments),
    label: 'Tidy',
  })
  const parsed = parseLabeledOutput(result.text, segments.length)
  if (!parsed) return null
  const accepted = parsed.map((block, index) => acceptCleanupResult(segments[index]!.text, block))
  if (accepted.some((block) => block == null)) return null
  return accepted as string[]
}

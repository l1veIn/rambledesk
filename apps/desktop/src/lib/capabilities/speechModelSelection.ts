import type { SpeechModelId } from '$lib/preferences'

/** Resolve a platform's saved choice without pretending an unsupported model is installed. */
export function resolveSupportedSpeechModelId(
  selected: SpeechModelId,
  models: readonly Readonly<{ id: SpeechModelId }>[],
): SpeechModelId | null {
  if (models.some((model) => model.id === selected)) return selected
  return models.length === 1 ? models[0].id : null
}

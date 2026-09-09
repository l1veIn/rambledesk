export type SpeechEditBuffer = { ids: string[]; text: string }

export function synchronizeSpeechEditBuffer(
  current: SpeechEditBuffer | null,
  edit: SpeechEditBuffer | null | undefined,
): SpeechEditBuffer | null {
  if (!edit) return null
  if (current && current.ids.length === edit.ids.length &&
    current.ids.every((id, index) => id === edit.ids[index])) return current
  return { ids: [...edit.ids], text: edit.text }
}

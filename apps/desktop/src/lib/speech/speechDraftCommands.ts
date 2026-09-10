import { get } from 'svelte/store'

import type { createSpeechDraftQueue } from './speechDraftQueue'

const speechDraftCommands = new Set([
  'accept-speech', 'discard-speech', 'tidy-speech', 'edit-speech',
  'save-speech-edit', 'cancel-speech-edit',
])

function validIds(value: unknown): value is string[] {
  return Array.isArray(value) && value.length > 0 &&
    value.every((id) => typeof id === 'string' && id.trim().length > 0) &&
    new Set(value).size === value.length
}

/** Consume speech-review commands from either window before general commands. */
export async function handleSpeechDraftCommand(
  queue: ReturnType<typeof createSpeechDraftQueue>,
  command: unknown,
  locked = false,
): Promise<boolean> {
  if (!command || typeof command !== 'object' || Array.isArray(command)) return false
  const payload = command as Record<string, unknown>
  const type = payload.type
  if (typeof type !== 'string' || !speechDraftCommands.has(type)) return false
  if (!validIds(payload.ids)) return true
  const ids = [...payload.ids]
  const edit = get(queue).edit
  const finishingEdit = type === 'save-speech-edit' || type === 'cancel-speech-edit'
  if (edit && !finishingEdit) return true
  if (locked && type !== 'discard-speech' && type !== 'cancel-speech-edit') return true
  if (finishingEdit && (!edit || edit.ids.length !== ids.length ||
    edit.ids.some((id, index) => id !== ids[index]))) return true

  switch (type) {
    case 'accept-speech': await queue.accept(ids); break
    case 'discard-speech': queue.discard(ids); break
    case 'tidy-speech': await queue.tidy(ids); break
    case 'edit-speech': queue.beginEdit(ids); break
    case 'cancel-speech-edit': queue.cancelEdit(ids); break
    case 'save-speech-edit':
      if (typeof payload.text === 'string' && payload.text.trim()) queue.saveEdit(ids, payload.text)
      break
  }
  return true
}

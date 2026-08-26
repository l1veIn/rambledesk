import { appendMarkdownBlock } from './feedbackText'
import type { FeedbackEditorHandle, SavePhase } from './types'

export type DraftSavePort = {
  save(input: {
    requestId: string
    body: string
    expectedRevision: number
  }): Promise<{ savedRevision: number }>
}

export type FeedbackDraftSession = {
  readonly requestId: string
  readonly generation: number
  readonly initialMarkdown: string
  markdown(): string
  savedRevision(): number
  savePhase(): SavePhase
  saveMessage(): string
  isDirty(): boolean
  isDisposed(): boolean
  applyUserEdit(markdown: string): void
  acknowledgeSave(savedMarkdown: string, savedRevision: number): void
  appendSpeech(text: string): void
  insertMarkdownBlock(markdown: string): void
  bindEditor(handle: FeedbackEditorHandle | null): void
  editor(): FeedbackEditorHandle | null
  saveNow(): Promise<boolean>
  settle(): Promise<boolean>
  dispose(): void
}

export function createFeedbackDraftSession(input: {
  requestId: string
  generation: number
  initialMarkdown: string
  initialRevision: number
  save: DraftSavePort
  onChange?: () => void
}): FeedbackDraftSession {
  const requestId = input.requestId
  const generation = input.generation
  const initialMarkdown = input.initialMarkdown
  let body = input.initialMarkdown
  let savedBody = input.initialMarkdown
  let revision = input.initialRevision
  let phase: SavePhase = input.initialMarkdown && input.initialRevision > 0 ? 'saved' : 'idle'
  let message = ''
  let disposed = false
  let editorHandle: FeedbackEditorHandle | null = null
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  let activeSave: Promise<boolean> | null = null

  function notify() {
    input.onChange?.()
  }

  function dirty() {
    return body !== savedBody
  }

  function cancelPendingSave() {
    if (saveTimer) {
      clearTimeout(saveTimer)
      saveTimer = undefined
    }
  }

  function scheduleSave(delayMs = 700) {
    if (disposed) return
    cancelPendingSave()
    saveTimer = setTimeout(() => void saveNow(), delayMs)
  }

  function applyUserEdit(markdown: string) {
    if (disposed || markdown === body) return
    body = markdown
    phase = dirty() ? 'unsaved' : 'saved'
    message = ''
    scheduleSave()
    notify()
  }

  function acknowledgeSave(savedMarkdown: string, savedRevision: number) {
    if (disposed) return
    savedBody = savedMarkdown
    revision = savedRevision
    phase = body === savedMarkdown ? 'saved' : 'unsaved'
    notify()
  }

  function writeBlock(markdown: string) {
    if (disposed) return
    const block = markdown.trim()
    if (!block) return
    const next = appendMarkdownBlock(body, block)
    if (editorHandle) editorHandle.applyExternalMarkdown(next)
    applyUserEdit(next)
  }

  function appendSpeech(text: string) {
    if (disposed) return
    const transcript = text.trim()
    if (!transcript) return
    if (editorHandle) {
      editorHandle.appendTranscript(transcript)
      return
    }
    writeBlock(transcript)
  }

  function bindEditor(handle: FeedbackEditorHandle | null) {
    if (disposed) return
    editorHandle = handle
  }

  async function saveNow(): Promise<boolean> {
    cancelPendingSave()
    if (disposed) return false
    if (!dirty()) return phase !== 'error'
    if (activeSave) {
      await activeSave
      if (disposed) return false
      return dirty() ? saveNow() : phase !== 'error'
    }

    const bodyToSave = body
    const revisionToSave = revision
    phase = 'saving'
    message = ''
    notify()

    activeSave = (async () => {
      try {
        const saved = await input.save.save({
          requestId,
          body: bodyToSave,
          expectedRevision: revisionToSave,
        })
        if (disposed) return false
        savedBody = bodyToSave
        revision = saved.savedRevision
        phase = body === bodyToSave ? 'saved' : 'unsaved'
        return true
      } catch (cause) {
        if (disposed) return false
        phase = 'error'
        message = cause instanceof Error ? cause.message : String(cause)
        return false
      }
    })()

    const succeeded = await activeSave
    activeSave = null
    notify()
    if (succeeded && !disposed && dirty()) return saveNow()
    return succeeded
  }

  function dispose() {
    if (disposed) return
    disposed = true
    cancelPendingSave()
    editorHandle = null
  }

  return {
    requestId,
    generation,
    initialMarkdown,
    markdown: () => body,
    savedRevision: () => revision,
    savePhase: () => phase,
    saveMessage: () => message,
    isDirty: dirty,
    isDisposed: () => disposed,
    applyUserEdit,
    acknowledgeSave,
    appendSpeech,
    insertMarkdownBlock: writeBlock,
    bindEditor,
    editor: () => editorHandle,
    saveNow,
    settle: saveNow,
    dispose,
  }
}

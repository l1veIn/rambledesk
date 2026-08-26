import {
  actionQuoteLines,
  actionQuoteMarkdown,
  appendMarkdownBlock,
  replaceLastOccurrence,
} from './feedbackText'
import {
  CLEANUP_CHAR_THRESHOLD,
  CLEANUP_TIMEOUT_MS,
  acceptCleanupResult,
  shouldStartCleanup,
  type CleanupTrigger,
} from './speechCleanupPolicy'
import type { FeedbackEditorHandle, SavePhase } from './types'

export type SpeechCleanupPort = {
  enabled: () => boolean
  clean: (text: string) => Promise<string>
  silenceMs?: number
  timeoutMs?: number
  schedule?: (fn: () => void, ms: number) => unknown
  cancel?: (id: unknown) => void
}

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
  insertActionQuote(index: number, instruction: string): void
  prepareNonSpeechInsert(): void
  isCleaning(): boolean
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
  cleanup?: SpeechCleanupPort
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
  let pendingPieces: string[] = []
  let cleaning = false
  let inflightCleanup: Promise<void> | null = null
  const schedule = input.cleanup?.schedule ?? ((fn: () => void, ms: number) => setTimeout(fn, ms))
  const timeoutMs = input.cleanup?.timeoutMs ?? CLEANUP_TIMEOUT_MS
  const TIMEOUT = Symbol('cleanup-timeout')

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
    if (editorHandle?.insertMarkdownAtCaret?.(block)) return
    applyUserEdit(appendMarkdownBlock(body, block))
  }

  function cleanupEnabled() {
    return input.cleanup?.enabled() === true
  }

  async function startCleanup(trigger: CleanupTrigger): Promise<void> {
    if (
      !shouldStartCleanup({
        enabled: cleanupEnabled(),
        busy: cleaning,
        pendingPieces,
        trigger,
      })
    ) {
      return
    }
    const pieces = pendingPieces
    pendingPieces = []
    cleaning = true
    notify()
    const editorAtStart = editorHandle
    const raw = editorAtStart?.beginSpeechCleanup?.() || pieces.join('\n\n')
    editorAtStart?.moveCursorAfterCleaningSpeech?.()
    inflightCleanup = (async () => {
      try {
        const work = input.cleanup!.clean(raw)
        const result = await Promise.race([
          work.then((text) => text.trim() || raw),
          new Promise<typeof TIMEOUT>((resolve) => {
            schedule(() => resolve(TIMEOUT), timeoutMs)
          }),
        ])
        if (disposed) return
        const accepted = result === TIMEOUT ? raw : acceptCleanupResult(raw, result)
        const replacedInEditor = editorAtStart?.isSpeechCleaning?.() === true
        editorAtStart?.finishSpeechCleanup?.(accepted === raw ? null : accepted)
        if (!replacedInEditor) {
          applyUserEdit(replaceLastOccurrence(body, raw, accepted))
        }
      } catch {
        if (!disposed) editorAtStart?.finishSpeechCleanup?.(null)
      } finally {
        cleaning = false
        inflightCleanup = null
        notify()
        if (!disposed) {
          if (
            shouldStartCleanup({
              enabled: cleanupEnabled(),
              busy: false,
              pendingPieces,
              trigger: 'stable-count',
            }) ||
            shouldStartCleanup({
              enabled: cleanupEnabled(),
              busy: false,
              pendingPieces,
              trigger: 'char-count',
            })
          ) {
            void startCleanup('stable-count')
          }
        }
      }
    })()
    await inflightCleanup
  }

  function prepareNonSpeechInsert() {
    void startCleanup('non-speech')
  }

  function appendSpeech(text: string) {
    if (disposed) return
    const transcript = text.trim()
    if (!transcript) return
    if (cleanupEnabled()) {
      pendingPieces = [...pendingPieces, transcript]
      if (editorHandle) editorHandle.appendTranscript(transcript, { pending: true })
      else applyUserEdit(appendMarkdownBlock(body, transcript))
      if (
        shouldStartCleanup({
          enabled: true,
          busy: cleaning,
          pendingPieces,
          trigger: pendingPieces.join('').length >= CLEANUP_CHAR_THRESHOLD ? 'char-count' : 'stable-count',
        })
      ) {
        void startCleanup(pendingPieces.join('').length >= CLEANUP_CHAR_THRESHOLD ? 'char-count' : 'stable-count')
      }
      return
    }
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

  async function settle(): Promise<boolean> {
    await startCleanup('settle')
    if (inflightCleanup) await inflightCleanup
    return saveNow()
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
    insertMarkdownBlock: (markdown: string) => {
      prepareNonSpeechInsert()
      writeBlock(markdown)
    },
    insertActionQuote: (index: number, instruction: string) => {
      prepareNonSpeechInsert()
      const lines = actionQuoteLines(index, instruction)
      if (editorHandle?.insertQuotedBlock?.(lines)) return
      writeBlock(actionQuoteMarkdown(index, instruction))
    },
    prepareNonSpeechInsert,
    isCleaning: () => cleaning,
    bindEditor,
    editor: () => editorHandle,
    saveNow,
    settle,
    dispose,
  }
}

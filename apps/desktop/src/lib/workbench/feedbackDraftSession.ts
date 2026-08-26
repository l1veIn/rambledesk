import {
  decodeFeedbackDraftDocument,
  snapshotFeedbackDraftDocument,
  snapshotFeedbackDraftMarkdown,
  updateFeedbackDraftDocument,
  type FeedbackDraftSnapshot,
} from '../feedbackDraftDocument'
import { CLEANED_SPEECH_NODE, PENDING_SPEECH_NODE } from '../pendingSpeech'
import { ACTION_CHANNEL_ATTR, stampActionIndex } from './actionChannel'
import {
  CLEANUP_CHAR_THRESHOLD,
  CLEANUP_TIMEOUT_MS,
  acceptCleanupResult,
  alignCleanupParts,
  pendingCharCount,
  shouldStartCleanup,
  type CleanupTrigger,
  type PendingSpeechSnapshot,
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
    documentJson: string
    bodyMarkdown: string
    expectedRevision: number
  }): Promise<{ savedRevision: number }>
}

export type FeedbackDraftSession = {
  readonly requestId: string
  readonly generation: number
  readonly initialDocumentJson: string
  readonly initialMarkdown: string
  documentJson(): string
  markdown(): string
  snapshot(): FeedbackDraftSnapshot
  savedRevision(): number
  savePhase(): SavePhase
  saveMessage(): string
  isDirty(): boolean
  isDisposed(): boolean
  applyUserEdit(snapshot: FeedbackDraftSnapshot): void
  acknowledgeSave(snapshot: FeedbackDraftSnapshot, savedRevision: number): void
  appendSpeech(text: string): void
  insertMarkdownBlock(markdown: string): void
  currentActionIndex(): number | null
  toggleActionChannel(index: number): void
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
  initialDocumentJson?: string | null
  initialMarkdown: string
  initialRevision: number
  save: DraftSavePort
  cleanup?: SpeechCleanupPort
  onChange?: () => void
}): FeedbackDraftSession {
  const requestId = input.requestId
  const generation = input.generation
  const restoredDocument = decodeFeedbackDraftDocument(input.initialDocumentJson)
  const initialSnapshot = restoredDocument
    ? snapshotFeedbackDraftDocument(restoredDocument)
    : snapshotFeedbackDraftMarkdown(input.initialMarkdown)
  const initialDocumentJson = initialSnapshot.documentJson
  const initialMarkdown = initialSnapshot.bodyMarkdown
  let draft = initialSnapshot
  let savedDraft = initialSnapshot
  let revision = input.initialRevision
  let phase: SavePhase = input.initialRevision > 0 ? 'saved' : 'idle'
  let message = ''
  let disposed = false
  let editorHandle: FeedbackEditorHandle | null = null
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  let activeSave: Promise<boolean> | null = null
  let pendingPieces: string[] = []
  let currentActionIndex: number | null = null
  let cleaning = false
  let inflightCleanup: Promise<void> | null = null
  const schedule = input.cleanup?.schedule ?? ((fn: () => void, ms: number) => setTimeout(fn, ms))
  const timeoutMs = input.cleanup?.timeoutMs ?? CLEANUP_TIMEOUT_MS
  const TIMEOUT = Symbol('cleanup-timeout')

  function notify() {
    input.onChange?.()
  }

  function dirty() {
    return (
      draft.documentJson !== savedDraft.documentJson ||
      draft.bodyMarkdown !== savedDraft.bodyMarkdown
    )
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

  function applyUserEdit(snapshot: FeedbackDraftSnapshot) {
    if (
      disposed ||
      (snapshot.documentJson === draft.documentJson && snapshot.bodyMarkdown === draft.bodyMarkdown)
    ) return
    draft = snapshot
    phase = dirty() ? 'unsaved' : 'saved'
    message = ''
    scheduleSave()
    notify()
  }

  function acknowledgeSave(snapshot: FeedbackDraftSnapshot, savedRevision: number) {
    if (disposed) return
    savedDraft = snapshot
    revision = savedRevision
    phase = dirty() ? 'unsaved' : 'saved'
    notify()
  }

  function cleanupEnabled() {
    return input.cleanup?.enabled() === true
  }

  function pendingSpeechSnapshot(): PendingSpeechSnapshot {
    const fromEditor = editorHandle?.pendingSpeech?.()
    if (fromEditor) return fromEditor
    return {
      count: pendingPieces.length,
      chars: pendingCharCount(pendingPieces),
      texts: pendingPieces,
    }
  }

  function appendNodes(nodes: NonNullable<ReturnType<typeof decodeFeedbackDraftDocument>>['content']) {
    if (!nodes?.length) return
    applyUserEdit(
      updateFeedbackDraftDocument(draft, (doc) => ({
        ...doc,
        content: [...(doc.content ?? []), ...nodes],
      })),
    )
  }

  function actionAttrs(extra: Record<string, unknown> = {}) {
    return currentActionIndex == null
      ? extra
      : { ...extra, [ACTION_CHANNEL_ATTR]: currentActionIndex }
  }

  function finishSpeechCleanupInDocument(pieces: string[], cleaned: string | null) {
    if (pieces.length === 0) return
    const parts = alignCleanupParts(pieces, cleaned)
    applyUserEdit(
      updateFeedbackDraftDocument(draft, (doc) => {
        const content = [...(doc.content ?? [])]
        const targets: number[] = []
        for (let index = content.length - 1; index >= 0 && targets.length < pieces.length; index -= 1) {
          const node = content[index]
          if (node.type === PENDING_SPEECH_NODE) targets.unshift(index)
        }
        targets.forEach((contentIndex, pieceIndex) => {
          const node = content[contentIndex]
          const attrs = { ...(node.attrs ?? {}) }
          delete attrs.status
          const text = parts?.[pieceIndex] ?? pieces[pieceIndex]
          content[contentIndex] = {
            type: cleaned == null ? 'paragraph' : CLEANED_SPEECH_NODE,
            ...(Object.keys(attrs).length > 0 ? { attrs } : {}),
            ...(text ? { content: [{ type: 'text', text }] } : {}),
          }
        })
        return { ...doc, content }
      }),
    )
  }

  async function startCleanup(trigger: CleanupTrigger): Promise<void> {
    const pending = pendingSpeechSnapshot()
    if (
      !shouldStartCleanup({
        enabled: cleanupEnabled(),
        busy: cleaning,
        pendingCount: pending.count,
        pendingChars: pending.chars,
        trigger,
      })
    ) return

    const pieces = pending.texts
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
        if (replacedInEditor) {
          editorAtStart?.finishSpeechCleanup?.(result === TIMEOUT ? null : accepted)
        } else {
          finishSpeechCleanupInDocument(pieces, result === TIMEOUT ? null : accepted)
        }
      } catch {
        if (!disposed) {
          if (editorAtStart?.isSpeechCleaning?.() === true) editorAtStart.finishSpeechCleanup?.(null)
          else finishSpeechCleanupInDocument(pieces, null)
        }
      } finally {
        cleaning = false
        inflightCleanup = null
        notify()
        if (!disposed) {
          const leftover = pendingSpeechSnapshot()
          if (
            shouldStartCleanup({
              enabled: cleanupEnabled(),
              busy: false,
              pendingCount: leftover.count,
              pendingChars: leftover.chars,
              trigger: 'stable-count',
            }) ||
            shouldStartCleanup({
              enabled: cleanupEnabled(),
              busy: false,
              pendingCount: leftover.count,
              pendingChars: leftover.chars,
              trigger: 'char-count',
            })
          ) void startCleanup('stable-count')
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
      else {
        appendNodes([
          {
            type: PENDING_SPEECH_NODE,
            attrs: actionAttrs({ status: 'pending' }),
            content: [{ type: 'text', text: transcript }],
          },
        ])
      }
      const pending = pendingSpeechSnapshot()
      const trigger = pending.chars >= CLEANUP_CHAR_THRESHOLD ? 'char-count' : 'stable-count'
      if (
        shouldStartCleanup({
          enabled: true,
          busy: cleaning,
          pendingCount: pending.count,
          pendingChars: pending.chars,
          trigger,
        })
      ) void startCleanup(trigger)
      return
    }
    if (editorHandle) {
      editorHandle.appendTranscript(transcript)
      return
    }
    appendNodes([
      {
        type: 'paragraph',
        attrs: actionAttrs(),
        content: [{ type: 'text', text: transcript }],
      },
    ])
  }

  function toggleActionChannel(index: number) {
    if (disposed) return
    currentActionIndex = currentActionIndex === index ? null : index
    editorHandle?.setActionChannel?.(currentActionIndex)
    notify()
  }

  function bindEditor(handle: FeedbackEditorHandle | null) {
    if (disposed) return
    editorHandle = handle
    editorHandle?.setActionChannel?.(currentActionIndex)
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

    const draftToSave = draft
    const revisionToSave = revision
    phase = 'saving'
    message = ''
    notify()

    activeSave = (async () => {
      try {
        const saved = await input.save.save({
          requestId,
          documentJson: draftToSave.documentJson,
          bodyMarkdown: draftToSave.bodyMarkdown,
          expectedRevision: revisionToSave,
        })
        if (disposed) return false
        savedDraft = draftToSave
        revision = saved.savedRevision
        phase = dirty() ? 'unsaved' : 'saved'
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

  function insertMarkdownBlock(markdown: string) {
    prepareNonSpeechInsert()
    if (editorHandle?.insertMarkdownAtCaret?.(markdown)) return
    const block = snapshotFeedbackDraftMarkdown(markdown)
    const parsed = decodeFeedbackDraftDocument(block.documentJson)
    const nodes = (parsed?.content ?? []).map((node) =>
      currentActionIndex == null ? node : stampActionIndex(node, currentActionIndex),
    )
    appendNodes(nodes)
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
    initialDocumentJson,
    initialMarkdown,
    documentJson: () => draft.documentJson,
    markdown: () => draft.bodyMarkdown,
    snapshot: () => draft,
    savedRevision: () => revision,
    savePhase: () => phase,
    saveMessage: () => message,
    isDirty: dirty,
    isDisposed: () => disposed,
    applyUserEdit,
    acknowledgeSave,
    appendSpeech,
    insertMarkdownBlock,
    currentActionIndex: () => currentActionIndex,
    toggleActionChannel,
    prepareNonSpeechInsert,
    isCleaning: () => cleaning,
    bindEditor,
    editor: () => editorHandle,
    saveNow,
    settle,
    dispose,
  }
}

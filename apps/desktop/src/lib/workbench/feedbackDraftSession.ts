import {
  decodeFeedbackDraftDocument,
  snapshotFeedbackDraftDocument,
  snapshotFeedbackDraftMarkdown,
  updateFeedbackDraftDocument,
  type FeedbackDraftSnapshot,
} from '../feedbackDraftDocument'
import {
  CLEANUP_STATE_ATTR,
  INPUT_SOURCE_ATTR,
  SPEECH_SEGMENT_ID_ATTR,
  asrParagraphAttrs,
  speechCleanupCandidates,
  type CleanupState,
  type SpeechCleanupSegment,
} from '../speechBlockMetadata'
import { ACTION_CHANNEL_ATTR, stampActionIndex } from './actionChannel'
import {
  DEFAULT_CLEANUP_CHAR_THRESHOLD,
  DEFAULT_CLEANUP_IDLE_MS,
  DEFAULT_CLEANUP_SEGMENT_THRESHOLD,
  DEFAULT_CLEANUP_TIMEOUT_MS,
  acceptCleanupResult,
  alignCleanupParts,
  shouldStartCleanup,
  type CleanupTrigger,
} from './speechCleanupPolicy'
import type { FeedbackEditorHandle, SavePhase } from './types'

export type SpeechCleanupPort = {
  enabled: () => boolean
  clean: (text: string) => Promise<string>
  settings?: () => SpeechCleanupSettings
  schedule?: (fn: () => void, ms: number) => unknown
  cancel?: (id: unknown) => void
}

export type SpeechCleanupSettings = {
  segmentThreshold: number
  charThreshold: number
  idleMs: number
  timeoutMs: number
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
  createSpeechSegmentId?: () => string
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
  let currentActionIndex: number | null = null
  let cleaning = false
  let inflightCleanup: Promise<void> | null = null
  let cleanupIdleTimer: unknown
  const schedule = input.cleanup?.schedule ?? ((fn: () => void, ms: number) => setTimeout(fn, ms))
  const cancel = input.cleanup?.cancel ?? ((id: unknown) => clearTimeout(id as ReturnType<typeof setTimeout>))
  const createSpeechSegmentId =
    input.createSpeechSegmentId ??
    (() => globalThis.crypto?.randomUUID?.() ?? `asr-${Date.now()}-${Math.random()}`)
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

  function cleanupSettings(): SpeechCleanupSettings {
    return input.cleanup?.settings?.() ?? {
      segmentThreshold: DEFAULT_CLEANUP_SEGMENT_THRESHOLD,
      charThreshold: DEFAULT_CLEANUP_CHAR_THRESHOLD,
      idleMs: DEFAULT_CLEANUP_IDLE_MS,
      timeoutMs: DEFAULT_CLEANUP_TIMEOUT_MS,
    }
  }

  function pendingCleanupSnapshot() {
    const doc = decodeFeedbackDraftDocument(draft.documentJson)
    const segments = doc ? speechCleanupCandidates(doc) : []
    return {
      count: segments.length,
      chars: segments.reduce((sum, segment) => sum + segment.text.length, 0),
      segments,
    }
  }

  function cancelCleanupIdleTimer() {
    if (cleanupIdleTimer === undefined) return
    cancel(cleanupIdleTimer)
    cleanupIdleTimer = undefined
  }

  function scheduleCleanupAfterIdle() {
    cancelCleanupIdleTimer()
    if (!cleanupEnabled() || cleaning || pendingCleanupSnapshot().count === 0) return
    cleanupIdleTimer = schedule(() => {
      cleanupIdleTimer = undefined
      void startCleanup('idle')
    }, cleanupSettings().idleMs)
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

  function finishSpeechCleanupInDocument(
    segments: SpeechCleanupSegment[],
    cleaned: string | null,
  ) {
    if (segments.length === 0) return
    const parts = alignCleanupParts(segments.map((segment) => segment.text), cleaned)
    const segmentIndex = new Map(
      segments.map((segment, index) => [segment.segmentId, { ...segment, index }]),
    )
    applyUserEdit(
      updateFeedbackDraftDocument(draft, (doc) => {
        function updateNode(node: typeof doc): typeof doc {
          const id = node.attrs?.[SPEECH_SEGMENT_ID_ATTR]
          const target = typeof id === 'string' ? segmentIndex.get(id) : undefined
          if (node.type === 'paragraph' && target) {
            const currentText = (node.content ?? [])
              .map((child) => child.text ?? '')
              .join('')
              .trim()
            const unchanged = currentText === target.text
            const state: CleanupState = !unchanged
              ? 'skipped'
              : cleaned == null
                ? 'failed'
                : 'cleaned'
            const text = cleaned != null && unchanged
              ? (parts?.[target.index] ?? currentText)
              : currentText
            return {
              ...node,
              attrs: {
                ...node.attrs,
                [INPUT_SOURCE_ATTR]: 'asr',
                [CLEANUP_STATE_ATTR]: state,
              },
              content: text ? [{ type: 'text', text }] : [],
            }
          }
          return node.content
            ? { ...node, content: node.content.map((child) => updateNode(child)) }
            : node
        }
        return updateNode(doc)
      }),
    )
  }

  async function startCleanup(trigger: CleanupTrigger): Promise<void> {
    const pending = pendingCleanupSnapshot()
    const settings = cleanupSettings()
    if (
      !shouldStartCleanup({
        enabled: cleanupEnabled(),
        busy: cleaning,
        pendingCount: pending.count,
        pendingChars: pending.chars,
        trigger,
        thresholds: settings,
      })
    ) return

    cancelCleanupIdleTimer()
    const segments = pending.segments
    cleaning = true
    notify()
    const editorAtStart = editorHandle
    editorAtStart?.beginSpeechCleanup?.(segments)
    const raw = segments.map((segment) => segment.text).join('\n\n')
    editorAtStart?.moveCursorAfterCleaningSpeech?.()
    inflightCleanup = (async () => {
      try {
        const work = input.cleanup!.clean(raw)
        const result = await Promise.race([
          work.then((text) => text.trim() || raw),
          new Promise<typeof TIMEOUT>((resolve) => {
            schedule(() => resolve(TIMEOUT), settings.timeoutMs)
          }),
        ])
        if (disposed) return
        const accepted = result === TIMEOUT ? raw : acceptCleanupResult(raw, result)
        const replacedInEditor = editorAtStart?.isSpeechCleaning?.() === true
        if (replacedInEditor) {
          editorAtStart?.finishSpeechCleanup?.(
            segments,
            result === TIMEOUT ? null : accepted,
          )
          const remaining = new Set(
            pendingCleanupSnapshot().segments.map((segment) => segment.segmentId),
          )
          if (segments.some((segment) => remaining.has(segment.segmentId))) {
            finishSpeechCleanupInDocument(
              segments,
              result === TIMEOUT ? null : accepted,
            )
          }
        } else {
          finishSpeechCleanupInDocument(segments, result === TIMEOUT ? null : accepted)
        }
      } catch {
        if (!disposed) {
          if (editorAtStart?.isSpeechCleaning?.() === true) {
            editorAtStart.finishSpeechCleanup?.(segments, null)
            const remaining = new Set(
              pendingCleanupSnapshot().segments.map((segment) => segment.segmentId),
            )
            if (segments.some((segment) => remaining.has(segment.segmentId))) {
              finishSpeechCleanupInDocument(segments, null)
            }
          } else {
            finishSpeechCleanupInDocument(segments, null)
          }
        }
      } finally {
        cleaning = false
        inflightCleanup = null
        notify()
        if (!disposed) {
          const leftover = pendingCleanupSnapshot()
          if (
            shouldStartCleanup({
              enabled: cleanupEnabled(),
              busy: false,
              pendingCount: leftover.count,
              pendingChars: leftover.chars,
              trigger: 'segment-count',
              thresholds: cleanupSettings(),
            }) ||
            shouldStartCleanup({
              enabled: cleanupEnabled(),
              busy: false,
              pendingCount: leftover.count,
              pendingChars: leftover.chars,
              trigger: 'char-count',
              thresholds: cleanupSettings(),
            })
          ) void startCleanup('segment-count')
          else scheduleCleanupAfterIdle()
        }
      }
    })()
    await inflightCleanup
  }

  function prepareNonSpeechInsert() {
    cancelCleanupIdleTimer()
    void startCleanup('non-speech')
  }

  function appendSpeech(text: string) {
    if (disposed) return
    const transcript = text.trim()
    if (!transcript) return
    const shouldCleanup = cleanupEnabled()
    const segmentId = createSpeechSegmentId()
    const state: CleanupState = shouldCleanup ? 'pending' : 'skipped'
    if (editorHandle) {
      editorHandle.appendTranscript(transcript, { asr: { segmentId, cleanupState: state } })
    } else {
      appendNodes([
        {
          type: 'paragraph',
          attrs: actionAttrs(asrParagraphAttrs(segmentId, state)),
          content: [{ type: 'text', text: transcript }],
        },
      ])
    }
    if (shouldCleanup) {
      const pending = pendingCleanupSnapshot()
      const settings = cleanupSettings()
      const trigger = pending.chars >= settings.charThreshold ? 'char-count' : 'segment-count'
      if (
        shouldStartCleanup({
          enabled: true,
          busy: cleaning,
          pendingCount: pending.count,
          pendingChars: pending.chars,
          trigger,
          thresholds: settings,
        })
      ) void startCleanup(trigger)
      else scheduleCleanupAfterIdle()
      return
    }
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
    cancelCleanupIdleTimer()
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
    cancelCleanupIdleTimer()
    editorHandle = null
  }

  scheduleCleanupAfterIdle()

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

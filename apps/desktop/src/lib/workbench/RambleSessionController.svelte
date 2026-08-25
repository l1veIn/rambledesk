<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { emitTo, listen } from '@tauri-apps/api/event'
  import { get } from 'svelte/store'
  import { onMount, tick } from 'svelte'

  import {
    clipboardCaptureLabel,
    eventBelongsToRamble,
    type ClipboardCaptureEvent,
  } from '../clipboardCapture'
  import { attachmentMarkdownUrl } from '../attachmentMarkdown'
  import type { FeedbackWorkspaceView } from '../feedback'
  import { t } from '../i18n'
  import { lightCleanupTranscript } from '../lightCleanup'
  import {
    cookingApiKey,
    cookingBaseUrl,
    cookingModel,
    cookingProvider,
    cookingReasoningEffort,
    lightCleanupEnabled,
    lightCleanupSystemPrompt,
    locale,
    notificationVolume,
    speechHotwords,
    speechInputDevice,
    speechModelId,
    speechVadSilenceMs,
    speechVadThreshold,
  } from '../preferences'
  import { playRecordArmSound } from '../notifications'
  import {
    RAMBLE_CONSOLE_COMMAND_EVENT,
    RAMBLE_CONSOLE_HIDE_EVENT,
    RAMBLE_CONSOLE_READY_EVENT,
    RAMBLE_CONSOLE_SHOW_EVENT,
    RAMBLE_CONSOLE_STATE_EVENT,
    type RambleConsoleCommand,
    type RambleConsoleState,
  } from '../rambleConsole'
  import {
    eventBelongsToVoiceSession,
    stableTranscript,
    voiceStartStillLive,
    type SpeechEvent,
    type VoiceRambleSessionView,
  } from '../speech'
  import {
    briefBlocks,
    findBriefBlock,
    joinTranscriptChunks,
    mergeLiveTranscript,
    rambleRequestIdAfterIdleNote,
  } from './briefNotes'
  import { createTranscriptPipeline } from './transcriptPipeline'
  import type { BriefNotePhase, FeedbackEditorHandle, RamblePhase, VoicePhase } from './types'

  export let isTauri = false
  export let workspace: FeedbackWorkspaceView | null = null
  export let interactionLocked = false
  export let editor: FeedbackEditorHandle | undefined
  export let attachmentBusy = false
  export let screenCaptureBusy = false
  export let attachmentMessage = ''
  export let voicePhase: VoicePhase = 'idle'
  export let voiceDevice = ''
  export let voicePartial = ''
  export let voiceLevel = 0
  export let voiceChunkIndex = 0
  export let voiceModelMissing = false
  export let ramblePhase: RamblePhase = 'idle'
  export let rambleStartedOnce = false
  export let rambleRequestId = ''
  export let rambleRequestTitle = ''
  export let rambleMessage = ''
  export let briefNotePhase: BriefNotePhase = 'idle'
  export let briefNoteBlockId: string | null = null
  export let onPageError: (message: string) => void = () => {}
  export let onRambleClipPending: (requestId: string, clipId: string) => void = () => {}
  export let onRambleClipReady: (requestId: string, text: string, clipId?: string) => void = () => {}
  export let onBriefNoteReady: (
    requestId: string,
    blockId: string,
    quote: string,
    note: string,
  ) => void = () => {}
  export let onSaveDraftNow: () => Promise<boolean> = async () => true
  export let onApplyWorkspaceMutation: (next: FeedbackWorkspaceView) => void = () => {}
  export let onRefreshAttachmentPreviews: (next: FeedbackWorkspaceView) => Promise<void> = async () => {}
  export let onStartScreenCapture: () => Promise<void> = async () => {}
  export let onImportAttachmentPaths: (paths: string[]) => Promise<void> = async () => {}
  export let onAppendRambleMarkdown: (requestId: string, markdown: string) => Promise<void> = async () => {}

  let voiceRequestId = ''
  let voiceSessionId = ''
  let rambleContextId = ''
  let rambleSourceLabel = ''
  let clipboardCaptureCount = 0
  let clipboardImageQueue: Promise<void> = Promise.resolve()
  let voiceSink: 'ramble' | 'brief-note' = 'ramble'
  let sessionChunks: string[] = []
  let briefNoteQuote = ''
  const transcriptPipeline = createTranscriptPipeline({
    cleanupEnabled: () => get(lightCleanupEnabled),
    cleanup: (text) =>
      lightCleanupTranscript(text, {
        provider: get(cookingProvider),
        apiKey: get(cookingApiKey),
        baseUrl: get(cookingBaseUrl),
        model: get(cookingModel),
        reasoningEffort: get(cookingReasoningEffort),
        locale: get(locale),
        systemPrompt: get(lightCleanupSystemPrompt),
      }),
    write: async (text) => {
      const requestId = voiceRequestId || rambleRequestId
      if (!requestId) return
      if (workspace?.request.request_id === requestId) editor?.appendTranscript(text)
      else await onAppendRambleMarkdown(requestId, text)
    },
    onError: (message) =>
      onPageError(t($locale, 'Light cleanup failed: {error}', { error: message })),
  })

  $: voiceActive =
    voicePhase === 'starting' ||
    voicePhase === 'listening' ||
    voicePhase === 'processing' ||
    voicePhase === 'stopping'
  $: voiceCanStop =
    voiceActive || (voicePhase === 'error' && voiceSessionId.length > 0)
  $: rambleActive = ramblePhase === 'active'
  $: rambleEngaged = ramblePhase !== 'idle'
  $: rambleBusy = ramblePhase === 'starting' || ramblePhase === 'stopping'
  $: rambleCanStop = rambleActive || voiceCanStop
  $: rambleCanExit = rambleEngaged || voiceCanStop
  $: if (rambleEngaged && workspace) {
    attachmentBusy
    screenCaptureBusy
    ramblePhase
    rambleBusy
    rambleActive
    rambleMessage
    voiceLevel
    voicePartial
    broadcastRambleConsoleState()
  }

  onMount(() => {
    if (!isTauri) return
    let voiceUnlisten: (() => void) | undefined
    let rambleShortcutUnlisten: (() => void) | undefined
    let captureShortcutUnlisten: (() => void) | undefined
    let consoleCommandUnlisten: (() => void) | undefined
    let consoleReadyUnlisten: (() => void) | undefined
    const onWindowKeydown = (event: KeyboardEvent) => {
      if (
        event.key.toLowerCase() === 'r' &&
        event.ctrlKey &&
        event.shiftKey &&
        !event.altKey &&
        !event.metaKey
      ) {
        event.preventDefault()
        void toggleRamble()
      }
    }
    window.addEventListener('keydown', onWindowKeydown, { capture: true })

    void listen<SpeechEvent>('voice-ramble-event', (event) => {
      handleVoiceEvent(event.payload)
    })
      .then((unlisten) => {
        voiceUnlisten = unlisten
      })
      .catch((cause) => {
        voicePhase = 'error'
        voiceMessage = t($locale, 'Cannot listen for speech events: {error}', { error: messageFrom(cause) })
      })
    void listen<string>('screen-capture-shortcut', () => {
      if (workspace && !interactionLocked) void onStartScreenCapture()
    })
      .then((unlisten) => {
        captureShortcutUnlisten = unlisten
      })
      .catch((cause) => {
        attachmentMessage = t($locale, 'Cannot listen for the capture shortcut: {error}', { error: messageFrom(cause) })
      })
    void listen<string>('ramble-toggle-shortcut', () => {
      void toggleRamble()
    })
      .then((unlisten) => {
        rambleShortcutUnlisten = unlisten
      })
      .catch((cause) => {
        ramblePhase = 'error'
        rambleMessage = t($locale, 'Cannot listen for the Ramble shortcut: {error}', { error: messageFrom(cause) })
      })
    void listen<RambleConsoleCommand>(RAMBLE_CONSOLE_COMMAND_EVENT, (event) => {
      void handleRambleConsoleCommand(event.payload)
    }).then((unlisten) => {
      consoleCommandUnlisten = unlisten
    })
    void listen(RAMBLE_CONSOLE_READY_EVENT, () => {
      if (rambleEngaged) void invoke('show_ramble_console').catch(() => {})
      broadcastRambleConsoleState()
    }).then((unlisten) => {
      consoleReadyUnlisten = unlisten
    })

    return () => {
      voiceUnlisten?.()
      rambleShortcutUnlisten?.()
      captureShortcutUnlisten?.()
      consoleCommandUnlisten?.()
      consoleReadyUnlisten?.()
      window.removeEventListener('keydown', onWindowKeydown, { capture: true })
      if (voiceCanStop) void invoke('stop_voice_ramble')
    }
  })

  export async function toggleRamble() {
    if (interactionLocked || rambleBusy || briefNotePhase === 'recording' || briefNotePhase === 'starting') {
      return
    }
    if (rambleActive || (voiceCanStop && voiceSink === 'ramble')) await stopRamble()
    else if (rambleEngaged) await resumeRamble()
    else await startRamble()
  }

  export async function toggleBriefNote(blockId: string) {
    if (interactionLocked || rambleBusy || briefNotePhase === 'starting') {
      return
    }
    if (briefNotePhase === 'recording') {
      if (briefNoteBlockId === blockId) await stopBriefNote()
      return
    }
    await startBriefNote(blockId)
  }

  export async function exitRamble() {
    if (!rambleCanExit && !rambleStartedOnce && briefNotePhase === 'idle') return
    if (briefNotePhase === 'recording' || briefNotePhase === 'starting') {
      await stopBriefNote()
    }
    if (rambleRequestId && rambleEngaged) {
      void invoke('record_diagnostic_event', {
        activity: 'ramble_stopped',
        caseId: rambleRequestId,
      }).catch(() => {})
    }
    if (voiceCanStop && voiceSink === 'ramble') {
      ramblePhase = 'stopping'
      rambleMessage = t($locale, 'Ending Ramble…')
      await stopVoiceRamble()
      await finalizeSession('ramble')
    }
    void invoke('hide_ramble_console').catch(() => {})
    void emitTo('ramble-console', RAMBLE_CONSOLE_HIDE_EVENT).catch(() => {})
    resetVoiceUi()
    resetRambleUi()
  }

  export async function importClipboardNow() {
    const requestId = rambleRequestId || workspace?.request.request_id || ''
    const contextId = rambleContextId || crypto.randomUUID()
    if (interactionLocked || !requestId || attachmentBusy) return
    attachmentMessage = ''
    try {
      const event = await invoke<ClipboardCaptureEvent>('capture_clipboard_once', {
        input: {
          request_id: requestId,
          ramble_context_id: contextId,
        },
      })
      handleClipboardCaptureEvent(event, requestId, contextId)
    } catch (cause) {
      attachmentMessage = t($locale, 'Could not import clipboard: {error}', { error: messageFrom(cause) })
    }
  }

  export function resetVoiceUi() {
    voicePhase = 'idle'
    voiceRequestId = ''
    voiceSessionId = ''
    voiceDevice = ''
    voicePartial = ''
    voiceLevel = 0
    voiceChunkIndex = 0
    voiceModelMissing = false
    voiceSink = 'ramble'
    sessionChunks = []
    briefNoteQuote = ''
  }

  export function resetRambleUi() {
    ramblePhase = 'idle'
    rambleStartedOnce = false
    rambleRequestId = ''
    rambleRequestTitle = ''
    rambleSourceLabel = ''
    rambleContextId = ''
    rambleMessage = ''
    clipboardCaptureCount = 0
    briefNotePhase = 'idle'
    briefNoteBlockId = null
    briefNoteQuote = ''
  }

  async function startRamble() {
    if (
      interactionLocked ||
      !workspace ||
      rambleBusy ||
      rambleEngaged ||
      workspace.request.status === 'completed' ||
      workspace.request.status === 'cancelled'
    ) {
      return
    }
    rambleStartedOnce = true
    rambleRequestId = workspace.request.request_id
    rambleRequestTitle = workspace.request.title
    rambleSourceLabel = workspace.request.source_hint ?? workspace.request.host_session_id
    rambleContextId = crypto.randomUUID()
    clipboardCaptureCount = 0
    ramblePhase = 'active'
    rambleMessage = t($locale, 'Starting the microphone…')
    void invoke('record_diagnostic_event', {
      activity: 'ramble_started',
      caseId: rambleRequestId,
    }).catch(() => {})
    void invoke('show_ramble_console').catch((cause) => {
      onPageError(t($locale, 'Could not open the Ramble console: {error}', { error: messageFrom(cause) }))
    })
    void emitTo('ramble-console', RAMBLE_CONSOLE_SHOW_EVENT).catch((cause) => {
      onPageError(t($locale, 'Could not open the Ramble console: {error}', { error: messageFrom(cause) }))
    })
    await beginVoiceRamble()
  }

  async function resumeRamble() {
    if (interactionLocked || !rambleRequestId || rambleActive || voiceActive || !rambleContextId) return
    await beginVoiceRamble()
  }

  async function recoverInterruptedRamble() {
    if (voiceCanStop) await stopVoiceRamble()
    await finalizeSession('ramble')
  }

  async function beginVoiceRamble() {
    if (sessionChunks.length > 0 && voiceSink === 'ramble') {
      const leftover = takeSessionTranscript()
      void deliverTranscript('ramble', leftover)
    }
    voiceSink = 'ramble'
    sessionChunks = []
    ramblePhase = 'active'
    rambleMessage = t($locale, 'Starting the microphone…')
    const voiceStarted = await startVoiceRamble()
    if (!voiceStarted || !voiceSessionId) {
      ramblePhase = 'error'
      rambleMessage = voiceMessage || t($locale, 'Microphone failed to start')
      return
    }

    rambleMessage = t($locale, 'Recording. Click to stop.')
  }

  async function stopRamble() {
    if (!rambleCanStop || ramblePhase === 'stopping' || voiceSink !== 'ramble') return
    const requestId = voiceRequestId || rambleRequestId
    const clipId = `ramble:${crypto.randomUUID()}`
    if (requestId) onRambleClipPending(requestId, clipId)
    ramblePhase = 'stopping'
    rambleMessage = t($locale, 'Finishing the final speech segment and pausing…')
    let stopError = ''
    if (voiceCanStop) {
      const voiceStopped = await stopVoiceRamble()
      if (!voiceStopped && !stopError) stopError = voiceMessage || t($locale, 'Microphone failed to stop')
    }
    const leftover = takeSessionTranscript()
    void deliverTranscript('ramble', leftover, { clipId, requestId })
    if (stopError) {
      ramblePhase = 'error'
      rambleMessage = stopError
    } else {
      ramblePhase = 'paused'
      rambleMessage = t($locale, 'Ramble paused; the document is preserved and capture tools remain available')
    }
  }

  async function startBriefNote(blockId: string) {
    if (
      interactionLocked ||
      !workspace ||
      workspace.request.status === 'completed' ||
      workspace.request.status === 'cancelled'
    ) {
      return
    }
    if (rambleActive) await stopRamble()
    if (voiceCanStop) await stopVoiceRamble()
    const requestId = workspace.request.request_id
    rambleRequestId = rambleRequestIdAfterIdleNote(ramblePhase, rambleRequestId, requestId)
    const block = findBriefBlock(
      briefBlocks({
        whatHappened: workspace.request.what_happened,
        actions: workspace.actions,
        contextRefs: workspace.context_refs,
      }),
      blockId,
    )
    briefNoteQuote = block?.quote ?? ''
    briefNotePhase = 'starting'
    briefNoteBlockId = blockId
    voiceSink = 'brief-note'
    sessionChunks = []
    const voiceStarted = await startVoiceRamble(requestId)
    if (!voiceStarted || !voiceSessionId) {
      briefNotePhase = 'error'
      briefNoteBlockId = null
      briefNoteQuote = ''
      voiceSink = 'ramble'
      onPageError(voiceMessage || t($locale, 'Microphone failed to start'))
      briefNotePhase = 'idle'
      return
    }
    briefNotePhase = 'recording'
  }

  function takeSessionTranscript(): string {
    const text = mergeLiveTranscript(sessionChunks, voicePartial)
    sessionChunks = []
    return text
  }

  async function deliverTranscript(
    sink: 'ramble' | 'brief-note',
    raw: string,
    meta?: { requestId?: string; blockId?: string; quote?: string; clipId?: string },
  ) {
    const requestId = meta?.requestId || voiceRequestId || rambleRequestId
    if (sink === 'ramble') {
      if (!raw) {
        if (requestId && meta?.clipId) onRambleClipReady(requestId, '', meta.clipId)
        return
      }
      try {
        const written = (await transcriptPipeline.prepare(raw)) || raw
        if (requestId) onRambleClipReady(requestId, written, meta?.clipId)
      } catch (cause) {
        onPageError(t($locale, 'Failed to write Ramble content: {error}', { error: messageFrom(cause) }))
        if (requestId) onRambleClipReady(requestId, raw, meta?.clipId)
      }
      return
    }
    if (!raw || !meta?.blockId || !requestId) return
    try {
      const written = (await transcriptPipeline.prepare(raw)) || raw
      onBriefNoteReady(requestId, meta.blockId, meta.quote ?? '', written)
    } catch (cause) {
      onPageError(t($locale, 'Failed to write Ramble content: {error}', { error: messageFrom(cause) }))
      onBriefNoteReady(requestId, meta.blockId, meta.quote ?? '', raw)
    }
  }

  async function stopBriefNote() {
    if (briefNotePhase !== 'recording' && briefNotePhase !== 'starting') return
    const blockId = briefNoteBlockId
    const quote = briefNoteQuote
    const requestId = voiceRequestId || rambleRequestId
    const snapshot = mergeLiveTranscript(sessionChunks, voicePartial)
    sessionChunks = []
    if (voiceCanStop) await stopVoiceRamble()
    const note = mergeLiveTranscript([snapshot], joinTranscriptChunks(sessionChunks))
    sessionChunks = []
    briefNotePhase = 'idle'
    briefNoteBlockId = null
    voiceSink = 'ramble'
    if (blockId && requestId && note) {
      onBriefNoteReady(requestId, blockId, quote, note)
      void transcriptPipeline.prepare(note).catch((cause) => {
        onPageError(t($locale, 'Failed to write Ramble content: {error}', { error: messageFrom(cause) }))
      })
    }
  }

  async function finalizeSession(sink: 'ramble' | 'brief-note'): Promise<string> {
    const raw = takeSessionTranscript()
    voiceSink = 'ramble'
    if (!raw) return ''
    await deliverTranscript(sink, raw)
    return raw
  }

  async function startVoiceRamble(requestId = rambleRequestId): Promise<boolean> {
    if (!requestId || voiceActive) return false
    voicePhase = 'starting'
    voiceRequestId = requestId
    voiceSessionId = ''
    voiceDevice = ''
    voicePartial = ''
    voiceMessage = t($locale, 'Connecting the microphone…')
    voiceLevel = 0
    voiceModelMissing = false
    void playRecordArmSound(get(notificationVolume))
    try {
      const session = await invoke<VoiceRambleSessionView>('start_voice_ramble', {
        input: {
          request_id: requestId,
          input_device: $speechInputDevice || null,
          model_id: $speechModelId,
          vad_threshold: $speechVadThreshold,
          vad_silence_ms: $speechVadSilenceMs,
          hotwords: $speechHotwords,
        },
      })
      if (!voiceStartStillLive(voicePhase)) {
        voiceSessionId = ''
        await invoke('stop_voice_ramble').catch(() => {})
        return false
      }
      voiceSessionId = session.voice_session_id
      if (voicePhase === 'starting') {
        voicePhase = 'listening'
        voiceMessage = t($locale, 'VAD is listening · Transcribes automatically after each spoken segment')
      }
    } catch (cause) {
      const message = messageFrom(cause)
      voicePhase = 'error'
      voiceMessage = message
      voiceModelMissing = /not installed|尚未安装/.test(message)
      return false
    }
    return true
  }

  async function stopVoiceRamble(): Promise<boolean> {
    if (!voiceCanStop) return true
    voicePhase = 'stopping'
    voiceMessage = t($locale, 'Finishing the final transcription segment…')
    try {
      await invoke('stop_voice_ramble')
      for (let attempt = 0; attempt < 5 && voicePhase === 'stopping'; attempt += 1) {
        await new Promise((resolve) => setTimeout(resolve, 20))
      }
      await tick()
      if (voicePhase === 'stopping') {
        voicePhase = 'idle'
        voiceMessage = t($locale, 'Recording stopped')
      }
    } catch (cause) {
      voicePhase = 'error'
      voiceMessage = messageFrom(cause)
      return false
    } finally {
      voiceLevel = 0
    }
    return true
  }

  function handleClipboardCaptureEvent(
    event: ClipboardCaptureEvent,
    currentRequestId: string,
    contextId: string,
  ) {
    if (
      interactionLocked ||
      !currentRequestId ||
      !eventBelongsToRamble(
        event,
        currentRequestId,
        contextId,
      )
    ) {
      if (event.type === 'image') {
        void invoke('discard_clipboard_capture_image', {
          captureId: event.capture_id,
        })
      }
      return
    }

    if (event.type === 'warning') {
      rambleMessage = event.message
      return
    }
    if (event.type === 'text') {
      const label = clipboardCaptureLabel(event.captured_at_ms, event.truncated, $locale)
      if (workspace?.request.request_id === currentRequestId) {
        editor?.appendClipboardCapture(event.text, label)
      } else {
        const quoted = event.text.split(/\r?\n/).map((line) => `> ${line}`).join('\n')
        void onAppendRambleMarkdown(currentRequestId, `> **${label}**\n>\n${quoted}`).catch(
          (cause) => onPageError(t($locale, 'Failed to write Ramble content: {error}', { error: messageFrom(cause) })),
        )
      }
      clipboardCaptureCount += 1
      rambleMessage = t($locale, 'Ramble active · {count} clipboard items captured', { count: clipboardCaptureCount })
      return
    }

    clipboardImageQueue = clipboardImageQueue
      .then(() => importClipboardImage(event))
      .catch((cause) => {
        attachmentMessage = t($locale, 'Could not insert clipboard image: {error}', { error: messageFrom(cause) })
      })
  }

  async function importClipboardImage(
    event: Extract<ClipboardCaptureEvent, { type: 'image' }>,
  ) {
    const requestId = event.request_id
    try {
      for (let attempt = 0; attachmentBusy && attempt < 200; attempt += 1) {
        await new Promise((resolve) => setTimeout(resolve, 50))
      }
      if (attachmentBusy) throw new Error(t($locale, 'The attachment channel is busy. Try importing the image again shortly.'))
      const visibleTarget = workspace?.request.request_id === requestId
      if (visibleTarget && !(await onSaveDraftNow())) {
        throw new Error(t($locale, 'The current draft could not be saved.'))
      }
      const target = visibleTarget
        ? workspace
        : await invoke<FeedbackWorkspaceView>('get_feedback_workspace', { requestId })
      if (!target) return

      attachmentBusy = true
      const next = await invoke<FeedbackWorkspaceView>('add_completed_clipboard_capture', {
        requestId,
        captureId: event.capture_id,
        rambleContextId: event.ramble_context_id,
        fileName: event.file_name,
        expectedRevision: target.draft.saved_revision,
      })
      const attachment = next.attachments.find(
        (item) => !target.attachments.some(
          (existing) => existing.attachment_id === item.attachment_id,
        ),
      )
      if (!attachment) throw new Error(t($locale, 'The image attachment was saved, but could not be inserted into the document flow.'))
      const label = clipboardCaptureLabel(event.captured_at_ms, false, $locale)
      if (visibleTarget && workspace?.request.request_id === requestId) {
        onApplyWorkspaceMutation(next)
        await onRefreshAttachmentPreviews(next)
        await tick()
        if (!editor?.appendCapturedAttachment(attachment, label)) {
          throw new Error(t($locale, 'The image attachment was saved, but could not be inserted into the document flow.'))
        }
        await onSaveDraftNow()
      } else {
        await onAppendRambleMarkdown(
          requestId,
          `> **${label}**\n\n![${attachment.file_name}](${attachmentMarkdownUrl(attachment.attachment_id)})`,
        )
      }
      clipboardCaptureCount += 1
      rambleMessage = t($locale, 'Ramble active · {count} clipboard items captured', { count: clipboardCaptureCount })
    } finally {
      attachmentBusy = false
      await invoke('discard_clipboard_capture_image', {
        captureId: event.capture_id,
      }).catch(() => {})
    }
  }

  function handleVoiceEvent(event: SpeechEvent) {
    const currentRequestId = voiceRequestId
    if (
      !eventBelongsToVoiceSession(
        event,
        currentRequestId,
        voiceSessionId,
      )
    ) {
      return
    }
    voiceRequestId = event.request_id
    voiceSessionId = event.voice_session_id
    switch (event.type) {
      case 'started':
        voicePhase = 'listening'
        voiceDevice = event.input_device
        voiceMessage = t($locale, 'Recording · {device}', { device: event.input_device })
        break
      case 'partial':
        voicePartial = event.text
        if (voicePhase !== 'stopping') voicePhase = 'listening'
        if (ramblePhase === 'active') rambleMessage = t($locale, 'Recording. Click to stop.')
        break
      case 'level':
        voiceLevel = Math.min(1, Math.max(0, event.rms * 8))
        if (voicePhase !== 'stopping') voicePhase = 'listening'
        if (ramblePhase === 'active') rambleMessage = t($locale, 'Recording. Click to stop.')
        break
      case 'processing':
        voiceChunkIndex = event.chunk_index + 1
        if (voicePhase !== 'stopping') voicePhase = 'processing'
        voiceMessage = t($locale, 'Transcribing segment {count}…', { count: event.chunk_index + 1 })
        break
      case 'stable': {
        const transcript = stableTranscript(event)
        if (
          transcript &&
          (briefNotePhase === 'recording' ||
            briefNotePhase === 'processing' ||
            briefNotePhase === 'starting' ||
            ramblePhase === 'active' ||
            ramblePhase === 'stopping' ||
            ramblePhase === 'error' ||
            !interactionLocked)
        ) {
          sessionChunks = [...sessionChunks, transcript]
        }
        voicePartial = transcript || voicePartial
        voiceChunkIndex = event.chunk_index + 1
        if (voicePhase !== 'stopping') voicePhase = 'listening'
        voiceMessage = t($locale, 'Segment {count} captured', { count: event.chunk_index + 1 })
        break
      }
      case 'warning':
        voiceMessage = event.message
        if (event.code === 'recognizer_loading' && (ramblePhase === 'active' || ramblePhase === 'starting')) {
          rambleMessage = t($locale, 'Recording. Speech recognition is warming up.')
        }
        break
      case 'stopped':
        voicePhase = 'idle'
        voiceSessionId = ''
        voiceLevel = 0
        voiceMessage = t($locale, 'Recording stopped')
        if (briefNotePhase === 'recording') {
          void stopBriefNote()
        } else if (ramblePhase === 'active') {
          ramblePhase = 'error'
          rambleMessage = t($locale, 'The microphone stopped unexpectedly; Ramble is paused')
          void recoverInterruptedRamble()
        }
        break
      case 'error':
        voicePhase = 'error'
        voiceLevel = 0
        voiceMessage = event.message
        if (briefNotePhase === 'recording' || briefNotePhase === 'starting') {
          onPageError(t($locale, 'Microphone error; Ramble is paused: {error}', { error: event.message }))
          void stopBriefNote()
        } else if (ramblePhase === 'active') {
          ramblePhase = 'error'
          rambleMessage = t($locale, 'Microphone error; Ramble is paused: {error}', { error: event.message })
          void recoverInterruptedRamble()
        }
        break
    }
  }

  async function handleRambleConsoleCommand(command: RambleConsoleCommand) {
    switch (command.type) {
      case 'toggle-recording':
        await toggleRamble()
        break
      case 'capture-screen':
        if (!interactionLocked) await onStartScreenCapture()
        break
      case 'import-clipboard':
        await importClipboardNow()
        break
      case 'import-files':
        if (!interactionLocked) await onImportAttachmentPaths(command.paths)
        break
      case 'exit':
        await exitRamble()
        break
    }
  }

  function broadcastRambleConsoleState() {
    if (!rambleEngaged || !rambleRequestId) return
    const state: RambleConsoleState = {
      phase:
        ramblePhase === 'active'
          ? 'recording'
          : ramblePhase === 'idle'
            ? 'paused'
            : ramblePhase,
      sourceLabel: rambleSourceLabel,
      requestTitle: rambleRequestTitle,
      recording: rambleActive,
      busy: rambleBusy,
      captureBusy: screenCaptureBusy,
      voiceLevel,
      partialTranscript: voicePartial,
      message: rambleMessage,
    }
    void emitTo('ramble-console', RAMBLE_CONSOLE_STATE_EVENT, state).catch(() => {})
  }

  let voiceMessage = ''

  function messageFrom(cause: unknown) {
    return cause instanceof Error ? cause.message : String(cause)
  }
</script>

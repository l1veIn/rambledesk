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
  import type { ApplicationTransport } from '../application/applicationTransport'
  import type { ActiveAction, DraftOperation } from '../draftOperations'
  import type { FeedbackWorkspaceView } from '../feedback'
  import { t } from '../i18n'
  import { playRecordArmSound } from '../notifications'
  import {
    locale,
    notificationVolume,
    speechHotwords,
    speechInputDevice,
    speechModelId,
    speechVadSilenceMs,
    speechVadThreshold,
  } from '../preferences'
  import { refreshShortcutSettings } from '../shortcutSettings'
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
    stableSpeechSegmentId,
    stableTranscript,
    voiceStartStillLive,
    type SpeechEvent,
    type VoiceRambleSessionView,
  } from '../speech'
  import { createSingleFlight } from '../singleFlight'
  import { resolvedRamblePhase } from './rambleSessionState'
  import type { RamblePhase, VoicePhase } from './types'

  export let isTauri = false
  export let transport: ApplicationTransport
  export let workspace: FeedbackWorkspaceView | null = null
  export let interactionLocked = false
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
  export let onPageError: (message: string) => void = () => {}
  export let onSaveDraftNow: () => Promise<boolean> = async () => true
  export let onApplyWorkspaceMutation: (next: FeedbackWorkspaceView) => void = () => {}
  export let onRefreshAttachmentPreviews: (next: FeedbackWorkspaceView) => Promise<void> = async () => {}
  export let onStartScreenCapture: () => Promise<void> = async () => {}
  export let onImportAttachmentPaths: (paths: string[]) => Promise<void> = async () => {}
  export let onRouteDraftOperation: (requestId: string, operation: DraftOperation) => Promise<void> = async () => {}
  export let getActiveAction: (requestId: string) => ActiveAction = () => null

  let voiceRequestId = ''
  let voiceSessionId = ''
  let rambleContextId = ''
  let rambleSourceLabel = ''
  let clipboardCaptureCount = 0
  let clipboardImageQueue: Promise<void> = Promise.resolve()
  const rambleTransition = createSingleFlight()

  $: voiceActive =
    voicePhase === 'starting' ||
    voicePhase === 'listening' ||
    voicePhase === 'processing' ||
    voicePhase === 'stopping'
  $: voiceCanStop =
    voiceActive || (voicePhase === 'error' && voiceSessionId.length > 0)
  $: visibleRamblePhase = resolvedRamblePhase(ramblePhase, voicePhase)
  $: rambleActive = visibleRamblePhase === 'active'
  $: rambleEngaged = visibleRamblePhase !== 'idle'
  $: rambleBusy = visibleRamblePhase === 'starting' || visibleRamblePhase === 'stopping'
  $: rambleCanStop = rambleActive || voiceCanStop
  $: rambleCanExit = rambleEngaged || voiceCanStop
  $: if (rambleEngaged && workspace) {
    attachmentBusy
    screenCaptureBusy
    visibleRamblePhase
    rambleBusy
    rambleActive
    rambleMessage
    voiceLevel
    voicePartial
    broadcastRambleConsoleState()
  }

  onMount(() => {
    if (!isTauri) return
    void refreshShortcutSettings()
    let disposed = false
    let voiceUnlisten: (() => void) | undefined
    let rambleShortcutUnlisten: (() => void) | undefined
    let captureShortcutUnlisten: (() => void) | undefined
    let consoleCommandUnlisten: (() => void) | undefined
    let consoleReadyUnlisten: (() => void) | undefined
    void listen<SpeechEvent>('voice-ramble-event', (event) => {
      handleVoiceEvent(event.payload)
    })
      .then((unlisten) => {
        if (disposed) unlisten()
        else voiceUnlisten = unlisten
      })
      .catch((cause) => {
        voicePhase = 'error'
        voiceMessage = t($locale, 'Cannot listen for speech events: {error}', { error: messageFrom(cause) })
      })
    void listen<string>('screen-capture-shortcut', () => {
      if (workspace && !interactionLocked) void onStartScreenCapture()
    })
      .then((unlisten) => {
        if (disposed) unlisten()
        else captureShortcutUnlisten = unlisten
      })
      .catch((cause) => {
        attachmentMessage = t($locale, 'Cannot listen for the capture shortcut: {error}', { error: messageFrom(cause) })
      })
    void listen<string>('ramble-toggle-shortcut', () => {
      void toggleRamble()
    })
      .then((unlisten) => {
        if (disposed) unlisten()
        else rambleShortcutUnlisten = unlisten
      })
      .catch((cause) => {
        ramblePhase = 'error'
        rambleMessage = t($locale, 'Cannot listen for the Ramble shortcut: {error}', { error: messageFrom(cause) })
      })
    void listen<RambleConsoleCommand>(RAMBLE_CONSOLE_COMMAND_EVENT, (event) => {
      void handleRambleConsoleCommand(event.payload)
    }).then((unlisten) => {
      if (disposed) unlisten()
      else consoleCommandUnlisten = unlisten
    })
    void listen(RAMBLE_CONSOLE_READY_EVENT, () => {
      if (rambleEngaged) void invoke('show_ramble_console').catch(() => {})
      broadcastRambleConsoleState()
    }).then((unlisten) => {
      if (disposed) unlisten()
      else consoleReadyUnlisten = unlisten
    })

    return () => {
      disposed = true
      voiceUnlisten?.()
      rambleShortcutUnlisten?.()
      captureShortcutUnlisten?.()
      consoleCommandUnlisten?.()
      consoleReadyUnlisten?.()
      if (voiceCanStop) void invoke('stop_voice_ramble')
    }
  })

  export function toggleRamble(): Promise<void> {
    return rambleTransition.run(async () => {
      if (interactionLocked || rambleBusy) return
      if (rambleActive || voiceCanStop) await stopRamble()
      else if (rambleEngaged) await resumeRamble()
      else await startRamble()
    })
  }

  export function exitRamble(): Promise<void> {
    return rambleTransition.run(async () => {
      if (!rambleCanExit && !rambleStartedOnce) return
      if (rambleRequestId) {
        void invoke('record_diagnostic_event', {
          activity: 'ramble_stopped',
          caseId: rambleRequestId,
        }).catch(() => {})
      }
      if (voiceCanStop) {
        ramblePhase = 'stopping'
        rambleMessage = t($locale, 'Ending Ramble…')
        await stopVoiceRamble()
      }
      void invoke('hide_ramble_console').catch(() => {})
      void emitTo('ramble-console', RAMBLE_CONSOLE_HIDE_EVENT).catch(() => {})
      resetVoiceUi()
      resetRambleUi()
    })
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
    ramblePhase = 'starting'
    rambleMessage = t($locale, 'Opening the Ramble console…')
    void invoke('record_diagnostic_event', {
      activity: 'ramble_started',
      caseId: rambleRequestId,
    }).catch(() => {})
    try {
      await invoke('show_ramble_console')
    } catch (cause) {
      onPageError(t($locale, 'Could not open the Ramble console: {error}', { error: messageFrom(cause) }))
    }
    void emitTo('ramble-console', RAMBLE_CONSOLE_SHOW_EVENT).catch((cause) => {
      onPageError(t($locale, 'Could not open the Ramble console: {error}', { error: messageFrom(cause) }))
    })
    await beginVoiceRamble()
  }

  async function resumeRamble() {
    if (interactionLocked || !rambleRequestId || rambleActive || voiceActive || !rambleContextId) return
    await beginVoiceRamble()
  }

  async function beginVoiceRamble() {
    ramblePhase = 'starting'
    rambleMessage = t($locale, 'Starting the microphone and live transcription…')
    const voiceStarted = await startVoiceRamble()
    if (!voiceStarted || !voiceSessionId) {
      ramblePhase = 'error'
      rambleMessage = voiceMessage || t($locale, 'Microphone failed to start')
      return
    }

    ramblePhase = 'active'
    rambleMessage = t($locale, 'Ramble active · Clipboard is read only when you click import')
  }

  async function stopRamble() {
    if (!rambleCanStop || ramblePhase === 'stopping') return
    ramblePhase = 'stopping'
    rambleMessage = t($locale, 'Finishing the final speech segment and pausing…')
    let stopError = ''
    if (voiceCanStop) {
      const voiceStopped = await stopVoiceRamble()
      if (!voiceStopped && !stopError) stopError = voiceMessage || t($locale, 'Microphone failed to stop')
    }
    if (stopError) {
      ramblePhase = 'error'
      rambleMessage = stopError
    } else {
      ramblePhase = 'paused'
      rambleMessage = t($locale, 'Ramble paused; the document is preserved and capture tools remain available')
    }
  }

  async function startVoiceRamble(): Promise<boolean> {
    if (!rambleRequestId || voiceActive) return false
    voicePhase = 'starting'
    voiceRequestId = rambleRequestId
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
          request_id: rambleRequestId,
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
      void onRouteDraftOperation(currentRequestId, {
        kind: 'appendClipboardText',
        text: event.text,
        label,
        action: getActiveAction(currentRequestId),
      }).catch(
        (cause) => onPageError(t($locale, 'Failed to write Ramble content: {error}', { error: messageFrom(cause) })),
      )
      clipboardCaptureCount += 1
      rambleMessage = t($locale, 'Ramble active · {count} clipboard items captured', { count: clipboardCaptureCount })
      return
    }

    const action = getActiveAction(currentRequestId)
    clipboardImageQueue = clipboardImageQueue
      .then(() => importClipboardImage(event, action))
      .catch((cause) => {
        attachmentMessage = t($locale, 'Could not insert clipboard image: {error}', { error: messageFrom(cause) })
      })
  }

  async function importClipboardImage(
    event: Extract<ClipboardCaptureEvent, { type: 'image' }>,
    action: ActiveAction,
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
        : await transport.call('getFeedbackWorkspace', { request_id: requestId })
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
      }
      await onRouteDraftOperation(requestId, {
        kind: 'appendAttachment',
        attachment,
        label,
        action,
      })
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
        markRambleRecording()
        voiceDevice = event.input_device
        voiceMessage = t($locale, 'Recording · {device}', { device: event.input_device })
        break
      case 'partial':
        voicePartial = event.text
        if (voicePhase !== 'stopping') voicePhase = 'listening'
        markRambleRecording()
        break
      case 'level':
        voiceLevel = Math.min(1, Math.max(0, event.rms * 8))
        if (voicePhase !== 'stopping') voicePhase = 'listening'
        markRambleRecording()
        break
      case 'processing':
        voiceChunkIndex = event.chunk_index + 1
        if (voicePhase !== 'stopping') voicePhase = 'processing'
        markRambleRecording()
        voiceMessage = t($locale, 'Transcribing segment {count}…', { count: event.chunk_index + 1 })
        break
      case 'stable': {
        const transcript = stableTranscript(event)
        if (transcript) {
          void onRouteDraftOperation(voiceRequestId, {
            kind: 'appendSpeech',
            segmentId: stableSpeechSegmentId(event),
            text: transcript,
            action: getActiveAction(voiceRequestId),
          }).catch(
            (cause) => onPageError(t($locale, 'Failed to write Ramble content: {error}', { error: messageFrom(cause) })),
          )
        }
        voicePartial = ''
        voiceChunkIndex = event.chunk_index + 1
        if (voicePhase !== 'stopping') voicePhase = 'listening'
        markRambleRecording()
        voiceMessage = t($locale, 'Segment {count} written to the document', { count: event.chunk_index + 1 })
        break
      }
      case 'warning':
        voiceMessage = event.message
        break
      case 'stopped':
        voicePhase = 'idle'
        voiceSessionId = ''
        voiceLevel = 0
        voicePartial = ''
        voiceMessage = t($locale, 'Recording stopped')
        if (ramblePhase === 'active') {
          ramblePhase = 'error'
          rambleMessage = t($locale, 'The microphone stopped unexpectedly; Ramble is paused')
        }
        break
      case 'error':
        voicePhase = 'error'
        voiceLevel = 0
        voicePartial = ''
        voiceMessage = event.message
        if (ramblePhase === 'active') {
          ramblePhase = 'error'
          rambleMessage = t($locale, 'Microphone error; Ramble is paused: {error}', { error: event.message })
        }
        break
    }
  }

  function markRambleRecording() {
    if (voicePhase === 'stopping' || ramblePhase === 'stopping' || ramblePhase === 'active') return
    ramblePhase = 'active'
    rambleMessage = t($locale, 'Ramble active · Clipboard is read only when you click import')
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
        visibleRamblePhase === 'active'
          ? 'recording'
          : visibleRamblePhase === 'idle'
            ? 'paused'
            : visibleRamblePhase,
      sourceLabel: rambleSourceLabel,
      requestTitle: rambleRequestTitle,
      recording: visibleRamblePhase === 'active',
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

<script lang="ts">
  import { get } from 'svelte/store'
  import { onMount, tick } from 'svelte'

  import { clipboardCaptureLabel } from '../clipboardCapture'
  import type { AttachmentCandidate } from '../capabilities/capturePlugin'
  import type { WorkbenchCapabilities } from '../capabilities/workbenchCapabilities'
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
  import { shortcutSettings } from '../shortcutSettings'
  import {
    type RambleConsoleCommand,
    type RambleConsoleState,
  } from '../rambleConsole'
  import {
    eventBelongsToSpeechSession,
    stableSpeechSegmentId,
    stableTranscript,
    voiceStartStillLive,
    type SpeechRecognitionEvent,
    type SpeechRecognitionSession,
  } from '../speech'
  import { createSingleFlight } from '../singleFlight'
  import { resolvedRamblePhase } from './rambleSessionState'
  import type { AttachmentCandidateTarget } from './attachmentController'
  import type { RamblePhase, VoicePhase } from './types'

  export let capabilities: Pick<
    WorkbenchCapabilities,
    'screenCapture' | 'clipboardCapture' | 'globalShortcuts' | 'speech' | 'rambleConsole'
  >
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
  export let onStartScreenCapture: () => Promise<void> = async () => {}
  export let onImportServerAttachmentPaths: (paths: string[]) => Promise<void> = async () => {}
  export let onPersistAttachmentCandidates: (
    target: AttachmentCandidateTarget,
    candidates: readonly AttachmentCandidate[],
  ) => Promise<boolean> = async (_target, candidates) => {
    await Promise.allSettled(candidates.map((candidate) => candidate.dispose()))
    return false
  }
  export let onRouteDraftOperation: (requestId: string, operation: DraftOperation) => Promise<void> = async () => {}
  export let getActiveAction: (requestId: string) => ActiveAction = () => null

  let voiceRequestId = ''
  let voiceSessionId = ''
  let speechSession: SpeechRecognitionSession | null = null
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
    let captureShortcutUnlisten = () => {}
    let rambleShortcutUnlisten = () => {}
    let consoleCommandUnlisten = () => {}
    let consoleReadyUnlisten = () => {}

    if (capabilities.globalShortcuts.status.availability !== 'unavailable') {
      void capabilities.globalShortcuts.implementation.read()
        .then((settings) => shortcutSettings.set(settings))
        .catch(() => {})
      rambleShortcutUnlisten = capabilities.globalShortcuts.implementation.onRambleToggle(() => {
        void toggleRamble()
      }, (cause) => {
          ramblePhase = 'error'
          rambleMessage = t($locale, 'Cannot listen for the Ramble shortcut: {error}', { error: messageFrom(cause) })
        })
    }
    if (capabilities.screenCapture.status.availability !== 'unavailable') {
      captureShortcutUnlisten = capabilities.screenCapture.implementation.onShortcut(() => {
        if (workspace && !interactionLocked) void onStartScreenCapture()
      }, (cause) => {
          attachmentMessage = t($locale, 'Cannot listen for the capture shortcut: {error}', { error: messageFrom(cause) })
        })
    }
    if (capabilities.rambleConsole.status.availability !== 'unavailable') {
      consoleCommandUnlisten = capabilities.rambleConsole.implementation.onCommand(
        (command) => void handleRambleConsoleCommand(command),
        () => {},
      )
      consoleReadyUnlisten = capabilities.rambleConsole.implementation.onReady(() => {
        if (rambleEngaged) {
          void capabilities.rambleConsole.implementation.restoreVisibility().catch(() => {})
        }
        broadcastRambleConsoleState()
      }, () => {})
    }

    return () => {
      rambleShortcutUnlisten()
      captureShortcutUnlisten()
      consoleCommandUnlisten()
      consoleReadyUnlisten()
      void speechSession?.cancel().catch(() => {})
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
        void capabilities.rambleConsole.implementation
          .recordDiagnostic('ramble_stopped', rambleRequestId)
          .catch(() => {})
      }
      if (voiceCanStop) {
        ramblePhase = 'stopping'
        rambleMessage = t($locale, 'Ending Ramble…')
        await stopVoiceRamble()
      }
      void capabilities.rambleConsole.implementation.hide().catch(() => {})
      resetVoiceUi()
      resetRambleUi()
    })
  }

  export async function importClipboardNow() {
    const requestId = workspace?.request.request_id || rambleRequestId || ''
    if (interactionLocked || !requestId || attachmentBusy) return
    const target: AttachmentCandidateTarget = {
      requestId,
      action: getActiveAction(requestId),
    }
    attachmentMessage = ''
    try {
      const result = await capabilities.clipboardCapture.implementation.captureOnce()
      handleClipboardCaptureResult(result, target)
    } catch (cause) {
      attachmentMessage = t($locale, 'Could not import clipboard: {error}', { error: messageFrom(cause) })
    }
  }

  export function resetVoiceUi() {
    voicePhase = 'idle'
    voiceRequestId = ''
    voiceSessionId = ''
    speechSession = null
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
    clipboardCaptureCount = 0
    ramblePhase = 'starting'
    rambleMessage = t($locale, 'Opening the Ramble console…')
    void capabilities.rambleConsole.implementation
      .recordDiagnostic('ramble_started', rambleRequestId)
      .catch(() => {})
    if (capabilities.rambleConsole.status.availability !== 'unavailable') {
      try {
        await capabilities.rambleConsole.implementation.show()
      } catch (cause) {
        onPageError(t($locale, 'Could not open the Ramble console: {error}', { error: messageFrom(cause) }))
      }
    }
    await beginVoiceRamble()
  }

  async function resumeRamble() {
    if (interactionLocked || !rambleRequestId || rambleActive || voiceActive) return
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
    if (!rambleRequestId || voiceActive || speechSession) return false
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
      const session = capabilities.speech.implementation.start(
        {
          inputDevice: $speechInputDevice || null,
          modelId: $speechModelId,
          vadThreshold: $speechVadThreshold,
          vadSilenceMs: $speechVadSilenceMs,
          hotwords: $speechHotwords,
        },
        {
          onEvent: handleVoiceEvent,
          onError: (cause) => {
            voicePhase = 'error'
            voiceMessage = t($locale, 'Cannot listen for speech events: {error}', { error: messageFrom(cause) })
          },
        },
      )
      speechSession = session
      voiceSessionId = session.id
      await session.ready
      if (!voiceStartStillLive(voicePhase)) {
        await session.cancel().catch(() => {})
        if (speechSession === session) speechSession = null
        voiceSessionId = ''
        return false
      }
      if (voicePhase === 'starting') {
        voicePhase = 'listening'
        voiceMessage = t($locale, 'VAD is listening · Transcribes automatically after each spoken segment')
      }
    } catch (cause) {
      speechSession = null
      voiceSessionId = ''
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
    const session = speechSession
    if (!session) {
      resetVoiceUi()
      return true
    }
    voicePhase = 'stopping'
    voiceMessage = t($locale, 'Finishing the final transcription segment…')
    try {
      await session.stop()
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

  function handleClipboardCaptureResult(
    result: Awaited<ReturnType<WorkbenchCapabilities['clipboardCapture']['implementation']['captureOnce']>>,
    target: AttachmentCandidateTarget,
  ) {
    if (interactionLocked || !target.requestId) {
      if (result.kind === 'attachment') void result.candidate.dispose().catch(() => {})
      return
    }

    const label = clipboardCaptureLabel(result.capturedAtMs, result.kind === 'text' && result.truncated, $locale)
    if (result.kind === 'text') {
      void onRouteDraftOperation(target.requestId, {
        kind: 'appendClipboardText',
        text: result.text,
        label,
        action: target.action,
      }).catch(
        (cause) => onPageError(t($locale, 'Failed to write Ramble content: {error}', { error: messageFrom(cause) })),
      )
      clipboardCaptureCount += 1
      rambleMessage = t($locale, 'Ramble active · {count} clipboard items captured', { count: clipboardCaptureCount })
      return
    }

    clipboardImageQueue = clipboardImageQueue
      .then(async () => {
        const persisted = await onPersistAttachmentCandidates(
          { ...target, label },
          [result.candidate],
        )
        if (!persisted) return
        clipboardCaptureCount += 1
        rambleMessage = t($locale, 'Ramble active · {count} clipboard items captured', { count: clipboardCaptureCount })
      })
      .catch((cause) => {
        attachmentMessage = t($locale, 'Could not insert clipboard image: {error}', { error: messageFrom(cause) })
      })
  }

  function handleVoiceEvent(event: SpeechRecognitionEvent) {
    const currentRequestId = voiceRequestId
    if (
      !currentRequestId ||
      !eventBelongsToSpeechSession(event, voiceSessionId)
    ) {
      return
    }
    switch (event.type) {
      case 'started':
        voicePhase = 'listening'
        markRambleRecording()
        voiceDevice = event.inputDevice
        voiceMessage = t($locale, 'Recording · {device}', { device: event.inputDevice })
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
        voiceChunkIndex = event.segmentIndex + 1
        if (voicePhase !== 'stopping') voicePhase = 'processing'
        markRambleRecording()
        voiceMessage = t($locale, 'Transcribing segment {count}…', { count: event.segmentIndex + 1 })
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
        voiceChunkIndex = event.segmentIndex + 1
        if (voicePhase !== 'stopping') voicePhase = 'listening'
        markRambleRecording()
        voiceMessage = t($locale, 'Segment {count} written to the document', { count: event.segmentIndex + 1 })
        break
      }
      case 'warning':
        voiceMessage = event.message
        break
      case 'stopped':
        voicePhase = 'idle'
        speechSession = null
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
      case 'import-server-paths':
        if (!interactionLocked) {
          await onImportServerAttachmentPaths(command.serverPaths)
        }
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
    void capabilities.rambleConsole.implementation.publish(state).catch(() => {})
  }

  let voiceMessage = ''

  function messageFrom(cause: unknown) {
    return cause instanceof Error ? cause.message : String(cause)
  }
</script>

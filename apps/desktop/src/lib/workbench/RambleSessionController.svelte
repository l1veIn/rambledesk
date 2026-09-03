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
  import RecordingOverlay from '../RecordingOverlay.svelte'
  import { selectedSpeechGroup, speechReviewCommand, type SpeechOverlayState } from '../speechOverlay'
  import { createSpeechDraftQueue, groupSpeechDrafts, type SpeechTarget } from './speechDraftQueue'
  import { createSpeechTargetTracker } from './speechTargetTracker'
  import {
    locale,
    notificationVolume,
    speechHotwords,
    speechInputDevice,
    speechConfirmBeforeWrite,
    speechOverlayEnabled,
    speechOverlayOpacity,
    speechModelId,
    speechVadSilenceMs,
    speechVadThreshold,
  } from '../preferences'
  import { matchesShortcut, shortcutSettings } from '../shortcutSettings'
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
  export let onOpenSpeechTarget: (requestId: string, segmentId?: string) => Promise<void> = async () => {}

  let voiceRequestId = ''
  let voiceSessionId = ''
  let speechSession: SpeechRecognitionSession | null = null
  let rambleSourceLabel = ''
  let clipboardCaptureCount = 0
  let clipboardImageQueue: Promise<void> = Promise.resolve()
  const rambleTransition = createSingleFlight()
  const speechDrafts = createSpeechDraftQueue({
    write: (requestId, operation) => onRouteDraftOperation(requestId, operation),
    storage: localStorage,
    onStorageError: () => onPageError(t($locale, 'Pending speech could not be saved on this device. Keep this window open until you review it.')),
  })
  const speechTargets = createSpeechTargetTracker(captureSpeechTarget)
  let receiptTimer: ReturnType<typeof setTimeout> | undefined
  let shownReceiptId = ''
  let nativeOverlayFailed = false
  let voiceMessage = ''
  let selectedGroupId: string | null = null
  let reviewOpen = false
  let shortcutsMounted = false
  let reviewShortcutKey = ''
  let reviewShortcutQueue = Promise.resolve()

  $: pendingSpeechGroups = groupSpeechDrafts($speechDrafts.drafts)
  $: speechReviewNeeded = pendingSpeechGroups.some((group) => !group.busy)
  $: selectedGroupId = selectedSpeechGroup({ groups: pendingSpeechGroups, selectedGroupId })?.ids[0] ?? null
  $: if (pendingSpeechGroups.length === 0 || $speechOverlayEnabled) reviewOpen = false
  $: speechOverlayState = {
    enabled: $speechOverlayEnabled,
    opacity: $speechOverlayOpacity,
    selectedGroupId,
    shortcuts: $shortcutSettings,
    phase: voicePhase,
    level: voiceLevel,
    partial: voicePartial,
    error: voicePhase === 'error' ? voiceMessage : '',
    target: rambleRequestId ? captureSpeechTarget() : null,
    groups: pendingSpeechGroups,
    receipt: $speechDrafts.receipt,
  } satisfies SpeechOverlayState
  $: if (shortcutsMounted) syncReviewShortcuts(speechReviewNeeded, $shortcutSettings.speechAccept, $shortcutSettings.speechDiscard)
  $: if (capabilities.rambleConsole.status.availability !== 'unavailable') {
    void capabilities.rambleConsole.implementation.publishSpeechOverlay(speechOverlayState)
      .catch(() => { nativeOverlayFailed = true })
  }
  $: if ($speechDrafts.receipt && $speechDrafts.receipt.id !== shownReceiptId) {
    shownReceiptId = $speechDrafts.receipt.id
    clearTimeout(receiptTimer)
    const receiptId = shownReceiptId
    receiptTimer = setTimeout(() => speechDrafts.clearReceipt(receiptId), 2600)
  }

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
    shortcutsMounted = true
    let captureShortcutUnlisten = () => {}
    let rambleShortcutUnlisten = () => {}
    let reviewShortcutUnlisten = () => {}
    let consoleCommandUnlisten = () => {}
    let consoleReadyUnlisten = () => {}
    let overlayReadyUnlisten = () => {}

    if (capabilities.globalShortcuts.status.availability !== 'unavailable') {
      reviewShortcutUnlisten = capabilities.globalShortcuts.implementation.onSpeechReview(
        handleSpeechReviewShortcut,
        (cause) => onPageError(t($locale, 'Speech confirmation shortcuts are unavailable: {error}', { error: messageFrom(cause) })),
      )
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
      overlayReadyUnlisten = capabilities.rambleConsole.implementation.onSpeechOverlayReady(() => {
        nativeOverlayFailed = false
        void capabilities.rambleConsole.implementation.publishSpeechOverlay(speechOverlayState)
          .catch(() => { nativeOverlayFailed = true })
      }, () => { nativeOverlayFailed = true })
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

    const onReviewKeydown = (event: KeyboardEvent) => {
      if (capabilities.globalShortcuts.status.availability !== 'unavailable' || event.defaultPrevented || event.repeat || event.isComposing) return
      const action = matchesShortcut(event, $shortcutSettings.speechAccept) ? 'accept'
        : matchesShortcut(event, $shortcutSettings.speechDiscard) ? 'discard' : null
      if (action && selectedSpeechGroup(speechOverlayState)) {
        event.preventDefault()
        handleSpeechReviewShortcut(action)
      }
    }
    window.addEventListener('keydown', onReviewKeydown)

    return () => {
      shortcutsMounted = false
      reviewShortcutUnlisten()
      window.removeEventListener('keydown', onReviewKeydown)
      syncReviewShortcuts(false, '', '')
      rambleShortcutUnlisten()
      captureShortcutUnlisten()
      consoleCommandUnlisten()
      consoleReadyUnlisten()
      overlayReadyUnlisten()
      clearTimeout(receiptTimer)
      void speechSession?.cancel().catch(() => {})
    }
  })

  function syncReviewShortcuts(active: boolean, accept: string, discard: string) {
    if (capabilities.globalShortcuts.status.availability === 'unavailable') return
    const key = `${active}:${accept}:${discard}`
    if (reviewShortcutKey === key) return
    reviewShortcutKey = key
    reviewShortcutQueue = reviewShortcutQueue
      .then(() => capabilities.globalShortcuts.implementation.setSpeechReviewActive(active))
      .catch((cause) => onPageError(t($locale, 'Speech confirmation shortcuts are unavailable: {error}', { error: messageFrom(cause) })))
  }

  function handleSpeechReviewShortcut(action: 'accept' | 'discard') {
    const command = speechReviewCommand(speechOverlayState, action, interactionLocked)
    if (command) void handleRambleConsoleCommand(command)
  }

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
        if (!(await stopVoiceRamble())) {
          ramblePhase = 'error'
          rambleMessage = voiceMessage
          return
        }
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
    speechTargets.reset()
  }

  export function hasPendingSpeech(requestId: string) {
    return speechDrafts.hasPending(requestId)
  }

  export function settleSpeechDrafts() {
    return speechDrafts.settled()
  }

  function captureSpeechTarget(): SpeechTarget {
    const action = getActiveAction(voiceRequestId || rambleRequestId)
    return { requestId: voiceRequestId || rambleRequestId, requestTitle: rambleRequestTitle,
      action: action ? { ...action } : null }
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
    speechTargets.reset()
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
      await speechDrafts.settled()
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
    return (voicePhase as VoicePhase) !== 'error'
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
    const speechTarget = speechTargets.observe(event)
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
        markRambleRecording()
        break
      case 'speech-started':
        if (voicePhase !== 'stopping') voicePhase = 'listening'
        break
      case 'processing':
        voiceChunkIndex = event.segmentIndex + 1
        if (voicePhase !== 'stopping') voicePhase = 'processing'
        markRambleRecording()
        voiceMessage = t($locale, 'Transcribing segment {count}…', { count: event.segmentIndex + 1 })
        break
      case 'stable': {
        const transcript = stableTranscript(event)
        const target = speechTarget ?? captureSpeechTarget()
        if (transcript) {
          speechDrafts.enqueue(stableSpeechSegmentId(event), transcript, target, $speechConfirmBeforeWrite)
        }
        voicePartial = ''
        voiceChunkIndex = event.segmentIndex + 1
        if (voicePhase !== 'stopping') voicePhase = 'listening'
        markRambleRecording()
        voiceMessage = t($locale, 'Listening…')
        break
      }
      case 'warning':
        voiceMessage = event.message
        break
      case 'stopped':
        if (event.reason === 'unexpected' || voicePhase === 'error') {
          if (voicePhase !== 'error') voiceMessage = t($locale, 'The microphone stopped unexpectedly; Ramble is paused')
          voicePhase = 'error'
        } else {
          voicePhase = 'idle'
          voiceMessage = t($locale, 'Recording stopped')
        }
        speechSession = null
        voiceSessionId = ''
        voiceLevel = 0
        voicePartial = ''
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
      case 'select-speech-group':
        if (pendingSpeechGroups.some((group) => group.ids.includes(command.id))) selectedGroupId = command.id
        break
      case 'accept-speech':
        if (!interactionLocked) await speechDrafts.accept(command.ids)
        break
      case 'discard-speech':
        speechDrafts.discard(command.ids)
        break
      case 'open-speech-target':
        await onOpenSpeechTarget(command.requestId, command.segmentId)
        break
      case 'retry-recording':
        await rambleTransition.run(async () => {
          if (interactionLocked || !rambleRequestId) return
          if (voiceCanStop) await stopVoiceRamble()
          await beginVoiceRamble()
        })
        break
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

  function messageFrom(cause: unknown) {
    return cause instanceof Error ? cause.message : String(cause)
  }
</script>

{#if capabilities.rambleConsole.status.availability === 'unavailable' || nativeOverlayFailed}
  <RecordingOverlay state={speechOverlayState} onCommand={(command) => void handleRambleConsoleCommand(command)} />
{/if}

{#if !$speechOverlayEnabled && pendingSpeechGroups.length > 0 && (speechReviewNeeded || reviewOpen)}
  <aside class="speech-review-dock" aria-label={t($locale, 'Pending speech groups')}>
    {#if reviewOpen}
      <div id="pending-speech-review">
        <RecordingOverlay state={{ ...speechOverlayState, enabled: true, opacity: 100 }} embedded draggable={false} onCommand={(command) => void handleRambleConsoleCommand(command)} />
      </div>
    {/if}
    <button class="review-toggle" aria-expanded={reviewOpen} aria-controls="pending-speech-review" onclick={() => reviewOpen = !reviewOpen}>
      {reviewOpen ? t($locale, 'Collapse transcript') : t($locale, 'Pending speech · {count}', { count: pendingSpeechGroups.length })}
    </button>
  </aside>
{/if}

<style>
  .speech-review-dock { position: fixed; right: 20px; bottom: 16px; z-index: 80; width: min(436px, calc(100vw - 32px)); pointer-events: none; display: flex; flex-direction: column; align-items: flex-end; }
  .speech-review-dock > div { width: 100%; }
  .review-toggle { pointer-events: auto; padding: 7px 12px; border: 1px solid var(--border); border-radius: 12px; background: var(--card); color: var(--foreground); box-shadow: 0 3px 12px #0002; font-size: 11px; cursor: pointer; }
  .review-toggle:focus-visible { outline: 2px solid var(--ring); outline-offset: 2px; }
</style>

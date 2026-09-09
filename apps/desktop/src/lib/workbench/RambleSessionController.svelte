<script lang="ts">
  import { onMount } from 'svelte'

  import { clipboardCaptureLabel } from '../clipboardCapture'
  import type { AttachmentCandidate } from '../capabilities/capturePlugin'
  import type { WorkbenchCapabilities } from '../capabilities/workbenchCapabilities'
  import type { ActiveAction, DraftOperation } from '../draftOperations'
  import type { FeedbackWorkspaceView } from '../feedback'
  import { t } from '../i18n'
  import RecordingOverlay from '../speech/RecordingOverlay.svelte'
  import { selectedSpeechGroup, speechReviewCommand, type SpeechOverlayState } from '../speech/speechOverlay'
  import { createSpeechDraftQueue, groupSpeechDrafts, type SpeechTarget } from './speechDraftQueue'
  import { handleSpeechDraftCommand } from './speechDraftCommands'
  import { tidySpeechSegments, type TidyConfig } from '../lightCleanup'
  import { createSpeechTargetTracker } from './speechTargetTracker'
  import {
    locale,
    notificationVolume,
    speechHotwords,
    speechInputDevice,
    speechConfirmBeforeWrite,
    speechAutoTidy,
    speechOverlayEnabled,
    speechOverlayOpacity,
    speechModelId,
    speechVadSilenceMs,
    speechVadThreshold,
  } from '../preferences'
  import { createVoiceRambleSession } from './voiceRambleSession'
  import { matchesShortcut, shortcutSettings } from '../settings/shortcutSettings'
  import {
    type RambleConsoleCommand,
    type RambleConsoleState,
  } from '../rambleConsole'
  import { createSingleFlight } from '../singleFlight'
  import { resolvedRamblePhase } from './rambleSessionState'
  import type { AttachmentCandidateTarget } from './attachmentController'
  import type { RamblePhase, VoicePhase } from './types'

  export let capabilities: Pick<
    WorkbenchCapabilities,
    'screenCapture' | 'clipboardCapture' | 'globalShortcuts' | 'speech' | 'rambleConsole'
  >
  export let workspace: FeedbackWorkspaceView | null = null
  export let tidyConfig: TidyConfig | null = null
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

  let rambleSourceLabel = ''
  let clipboardCaptureCount = 0
  let clipboardImageQueue: Promise<void> = Promise.resolve()
  const rambleTransition = createSingleFlight()
  const speechDrafts = createSpeechDraftQueue({
    write: (requestId, operation) => onRouteDraftOperation(requestId, operation),
    tidy: async (text) => {
      if (!tidyConfig) throw new Error(t($locale, 'Configure Tidy in Post-processing settings before tidying speech.'))
      const result = await tidySpeechSegments([{ segmentId: 'speech-review', text }], tidyConfig)
      if (!result?.[0]?.trim()) throw new Error(t($locale, 'Tidy returned no usable text. Your original transcript has been kept.'))
      return result[0]
    },
    storage: localStorage,
    onStorageError: () => onPageError(t($locale, 'Pending speech could not be saved on this device. Keep this window open until you review it.')),
  })
  const speechTargets = createSpeechTargetTracker(captureSpeechTarget)
  const voice = createVoiceRambleSession({
    speech: capabilities.speech,
    tr: (source, values) => t($locale, source, values),
    messageFrom,
    getInputDevice: () => $speechInputDevice,
    getModelId: () => $speechModelId,
    getVadThreshold: () => $speechVadThreshold,
    getVadSilenceMs: () => $speechVadSilenceMs,
    getHotwords: () => $speechHotwords,
    getNotificationVolume: () => $notificationVolume,
    resolveTarget: (event) => speechTargets.observe(event) ?? captureSpeechTarget(),
    resetTargets: () => speechTargets.reset(),
    onStable: (segmentId, transcript, target) =>
      speechDrafts.enqueue(segmentId, transcript, target, $speechConfirmBeforeWrite, $speechAutoTidy),
    onRecording: markRambleRecording,
    onMicrophoneStopped: (message) => {
      if (ramblePhase !== 'active') return
      ramblePhase = 'error'
      rambleMessage = message
    },
    onMicrophoneError: (message) => {
      if (ramblePhase !== 'active') return
      ramblePhase = 'error'
      rambleMessage = t($locale, 'Microphone error; Ramble is paused: {error}', { error: message })
    },
    waitForDrafts: () => speechDrafts.settled(),
  })

  // The controller owns the microphone; these mirror it into the bound props.
  $: voicePhase = $voice.phase
  $: voiceDevice = $voice.device
  $: voicePartial = $voice.partial
  $: voiceLevel = $voice.level
  $: voiceChunkIndex = $voice.chunkIndex
  $: voiceModelMissing = $voice.modelMissing
  let receiptTimer: ReturnType<typeof setTimeout> | undefined
  let shownReceiptId = ''
  let nativeOverlayFailed = false
  let selectedGroupId: string | null = null
  let reviewOpen = false
  let shortcutsMounted = false
  let reviewShortcutKey = ''
  let reviewShortcutQueue = Promise.resolve()

  $: pendingSpeechGroups = groupSpeechDrafts($speechDrafts.drafts)
  $: speechReviewNeeded = !$speechDrafts.edit && pendingSpeechGroups.some((group) => !group.busy)
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
    error: voicePhase === 'error' ? $voice.message : '',
    target: rambleRequestId ? captureSpeechTarget() : null,
    groups: pendingSpeechGroups,
    receipt: $speechDrafts.receipt,
    edit: $speechDrafts.edit,
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
    $voice.phase === 'starting' ||
    $voice.phase === 'listening' ||
    $voice.phase === 'processing' ||
    $voice.phase === 'stopping'
  $: voiceCanStop =
    voiceActive || ($voice.phase === 'error' && $voice.sessionId.length > 0)
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
      speechDrafts.dispose()
      reviewShortcutUnlisten()
      window.removeEventListener('keydown', onReviewKeydown)
      syncReviewShortcuts(false, '', '')
      rambleShortcutUnlisten()
      captureShortcutUnlisten()
      consoleCommandUnlisten()
      consoleReadyUnlisten()
      overlayReadyUnlisten()
      clearTimeout(receiptTimer)
      void voice.cancel()
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
        if (!(await voice.stop())) {
          ramblePhase = 'error'
          rambleMessage = $voice.message
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
    voice.reset()
  }

  export function hasPendingSpeech(requestId: string) {
    return speechDrafts.hasPending(requestId)
  }

  export function settleSpeechDrafts() {
    return speechDrafts.settled()
  }

  function captureSpeechTarget(): SpeechTarget {
    const action = getActiveAction($voice.requestId || rambleRequestId)
    return { requestId: $voice.requestId || rambleRequestId, requestTitle: rambleRequestTitle,
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
    const voiceStarted = await voice.start(rambleRequestId)
    if (!voiceStarted || !$voice.sessionId) {
      ramblePhase = 'error'
      rambleMessage = $voice.message || t($locale, 'Microphone failed to start')
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
      const voiceStopped = await voice.stop()
      if (!voiceStopped && !stopError) stopError = $voice.message || t($locale, 'Microphone failed to stop')
    }
    if (stopError) {
      ramblePhase = 'error'
      rambleMessage = stopError
    } else {
      ramblePhase = 'paused'
      rambleMessage = t($locale, 'Ramble paused; the document is preserved and capture tools remain available')
    }
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

  function markRambleRecording() {
    if (voicePhase === 'stopping' || ramblePhase === 'stopping' || ramblePhase === 'active') return
    ramblePhase = 'active'
    rambleMessage = t($locale, 'Ramble active · Clipboard is read only when you click import')
  }

  async function handleRambleConsoleCommand(command: RambleConsoleCommand) {
    if (await handleSpeechDraftCommand(speechDrafts, command, interactionLocked)) return
    switch (command.type) {
      case 'select-speech-group':
        if (!$speechDrafts.edit && pendingSpeechGroups.some((group) => group.ids.includes(command.id))) selectedGroupId = command.id
        break
      case 'open-speech-target':
        await onOpenSpeechTarget(command.requestId, command.segmentId)
        break
      case 'retry-recording':
        await rambleTransition.run(async () => {
          if (interactionLocked || !rambleRequestId) return
          if (voiceCanStop) await voice.stop()
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

{#if !$speechOverlayEnabled && pendingSpeechGroups.length > 0 && (speechReviewNeeded || reviewOpen || $speechDrafts.edit)}
  <aside class="speech-review-dock" aria-label={t($locale, 'Pending speech groups')}>
    {#if reviewOpen || $speechDrafts.edit}
      <div id="pending-speech-review">
        <RecordingOverlay state={{ ...speechOverlayState, enabled: true, opacity: 100 }} embedded draggable={false} onCommand={(command) => void handleRambleConsoleCommand(command)} />
      </div>
    {/if}
    <button class="review-toggle" disabled={!!$speechDrafts.edit} aria-expanded={reviewOpen || !!$speechDrafts.edit} aria-controls="pending-speech-review" onclick={() => reviewOpen = !reviewOpen}>
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

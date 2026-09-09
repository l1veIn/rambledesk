<script lang="ts">
  import { onMount } from 'svelte'
  import type { FeedbackPreparation } from '../speech/rambleSessionControllerHandle'

  import { clipboardCaptureLabel } from '../clipboardCapture'
  import type { AttachmentCandidate } from '../capabilities/capturePlugin'
  import type { WorkbenchCapabilities } from '../capabilities/workbenchCapabilities'
  import type { ActiveAction, DraftOperation } from '../draftOperations'
  import type { FeedbackWorkspaceView } from '../feedback'
  import { t } from '../i18n'
  import RecordingOverlay from '../speech/RecordingOverlay.svelte'
  import { selectedSpeechGroup, speechReviewCommand, type SpeechOverlayState } from '../speech/speechOverlay'
  import { createSpeechDraftQueue, groupSpeechDrafts, type SpeechTarget } from '../speech/speechDraftQueue'
  import { handleSpeechDraftCommand } from '../speech/speechDraftCommands'
  import { tidySpeechSegments, type TidyConfig } from '../lightCleanup'
  import { createSpeechTargetTracker } from '../speech/speechTargetTracker'
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
  import { createRambleSession } from './rambleSession'
  import { matchesShortcut, shortcutSettings } from '../settings/shortcutSettings'
  import {
    type RambleConsoleCommand,
    type RambleConsoleState,
  } from '../rambleConsole'
  import { createSingleFlight } from '../singleFlight'
  import type { AttachmentCandidateTarget } from './attachmentController'

  export let capabilities: Pick<
    WorkbenchCapabilities,
    'screenCapture' | 'clipboardCapture' | 'globalShortcuts' | 'speech' | 'rambleConsole'
  >
  export let workspace: FeedbackWorkspaceView | null = null
  export let tidyConfig: TidyConfig | null = null
  export let interactionLocked = false
  export let attachmentBusy = false
  export let screenCaptureBusy = false
  export let onAttachmentMessage: (message: string) => void = () => {}
  export let session = createRambleSession()
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
  export let waitForDocumentWrites: () => Promise<void> = async () => {}
  export let getActiveAction: (requestId: string) => ActiveAction = () => null
  export let onOpenSpeechTarget: (requestId: string, segmentId?: string) => Promise<void> = async () => {}

  let rambleSourceLabel = ''
  let disposed = false
  let clipboardCapture: Promise<void> | null = null
  let clipboardFailure = ''
  let clipboardCaptureId = 0
  let lastClipboardFailure: { captureId: number; message: string } | null = null
  let clipboardCaptureCount = 0
  let clipboardImageQueue: Promise<void> = Promise.resolve()
  const rambleTransition = createSingleFlight()
  let exitFlight: Promise<void> | null = null
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
  const voice = session.connectVoice({
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
      if ($session.phase !== 'active') return
      session.transition('error', message)
    },
    onMicrophoneError: (message) => {
      if ($session.phase !== 'active') return
      session.transition('error', t($locale, 'Microphone error; Ramble is paused: {error}', { error: message }))
    },
    waitForDrafts: () => speechDrafts.settled(),
  })

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
    phase: $voice.phase,
    level: $voice.level,
    partial: $voice.partial,
    error: $voice.phase === 'error' ? $voice.message : '',
    target: $session.requestId ? captureSpeechTarget() : null,
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

  $: voiceActive = $session.voiceActive
  $: voiceCanStop = $session.voiceCanStop
  $: visibleRamblePhase = $session.visiblePhase
  $: rambleActive = $session.active
  $: rambleEngaged = $session.engaged
  $: rambleBusy = visibleRamblePhase === 'starting' || visibleRamblePhase === 'stopping'
  $: rambleCanStop = rambleActive || voiceCanStop
  $: rambleCanExit = rambleEngaged || voiceCanStop
  $: if (rambleEngaged && workspace) {
    attachmentBusy
    screenCaptureBusy
    visibleRamblePhase
    rambleBusy
    rambleActive
    $session.message
    $voice.level
    $voice.partial
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
          session.transition('error', t($locale, 'Cannot listen for the Ramble shortcut: {error}', { error: messageFrom(cause) }))
        })
    }
    if (capabilities.screenCapture.status.availability !== 'unavailable') {
      captureShortcutUnlisten = capabilities.screenCapture.implementation.onShortcut(() => {
        if (workspace && !interactionLocked) void onStartScreenCapture()
      }, (cause) => {
          onAttachmentMessage(t($locale, 'Cannot listen for the capture shortcut: {error}', { error: messageFrom(cause) }))
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
      disposed = true
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
    if (exitFlight) return exitFlight
    return rambleTransition.run(async () => {
      if (interactionLocked || rambleBusy) return
      if (rambleActive || voiceCanStop) await stopRamble()
      else if (rambleEngaged) await resumeRamble()
      else await startRamble()
    })
  }

  export function exitRamble(): Promise<void> {
    if (exitFlight) return exitFlight
    exitFlight = Promise.resolve().then(finishRamble).finally(() => { exitFlight = null })
    return exitFlight
  }

  async function finishRamble() {
    // Joining a start is only a barrier: ending the session is a distinct intent
    // and must still run after the microphone has actually become ready.
    await rambleTransition.run(async () => {})
    await rambleTransition.run(async () => {
      if (!rambleCanExit && !$session.startedOnce) return
      if ($session.requestId) {
        void capabilities.rambleConsole.implementation
          .recordDiagnostic('ramble_stopped', $session.requestId)
          .catch(() => {})
      }
      if (voiceCanStop) {
        session.transition('stopping', t($locale, 'Ending Ramble…'))
        if (!(await voice.stop())) {
          session.transition('error', $voice.message)
          return
        }
      }
      void capabilities.rambleConsole.implementation.hide().catch(() => {})
      resetVoiceUi()
      resetRambleUi()
    })
    // Imports remain available while a prior write drains. Include any import
    // accepted during that wait before reporting the input boundary as settled.
    do {
      await clipboardCapture
      await speechDrafts.settled()
      await clipboardImageQueue
      await waitForDocumentWrites()
    } while (clipboardCapture)
  }

  export function importClipboardNow(): Promise<void> {
    if (clipboardCapture) return clipboardCapture
    clipboardFailure = ''
    const captureId = ++clipboardCaptureId
    clipboardCapture = Promise.resolve().then(captureClipboard).finally(() => {
      if (clipboardFailure) lastClipboardFailure = { captureId, message: clipboardFailure }
      clipboardCapture = null
    })
    return clipboardCapture
  }

  async function captureClipboard() {
    const requestId = workspace?.request.request_id || $session.requestId || ''
    if (disposed || interactionLocked || !requestId || attachmentBusy) return
    const target: AttachmentCandidateTarget = {
      requestId,
      action: getActiveAction(requestId),
    }
    onAttachmentMessage('')
    try {
      const result = await capabilities.clipboardCapture.implementation.captureOnce()
      await handleClipboardCaptureResult(result, target)
    } catch (cause) {
      clipboardFailure = t($locale, 'Could not import clipboard: {error}', { error: messageFrom(cause) })
      if (!disposed) onAttachmentMessage(clipboardFailure)
    }
  }

  function resetVoiceUi() {
    voice.reset()
  }

  export async function prepareFeedback(requestId: string): Promise<FeedbackPreparation> {
    const firstCaptureId = clipboardCapture ? clipboardCaptureId : clipboardCaptureId + 1
    await exitRamble()
    if (lastClipboardFailure && lastClipboardFailure.captureId >= firstCaptureId) {
      return { kind: 'failed', message: lastClipboardFailure.message }
    }
    const message = session.speechStopError()
    if (message) return { kind: 'failed', message }
    return speechDrafts.hasPending(requestId) ? { kind: 'pending-speech' } : { kind: 'ready' }
  }

  function captureSpeechTarget(): SpeechTarget {
    const action = getActiveAction($voice.requestId || $session.requestId)
    return { requestId: $voice.requestId || $session.requestId, requestTitle: $session.requestTitle,
      action: action ? { ...action } : null }
  }

  function resetRambleUi() {
    session.reset()
    rambleSourceLabel = ''
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
    session.begin(workspace.request)
    rambleSourceLabel = workspace.request.source_hint ?? workspace.request.host_session_id
    clipboardCaptureCount = 0
    session.transition('starting', t($locale, 'Opening the Ramble console…'))
    void capabilities.rambleConsole.implementation
      .recordDiagnostic('ramble_started', $session.requestId)
      .catch(() => {})
    if (capabilities.rambleConsole.status.availability !== 'unavailable') {
      try {
        await capabilities.rambleConsole.implementation.show()
      } catch (cause) {
        onPageError(t($locale, 'Could not open the Ramble console: {error}', { error: messageFrom(cause) }))
      }
    }
    if (!disposed) await beginVoiceRamble()
  }

  async function resumeRamble() {
    if (interactionLocked || !$session.requestId || rambleActive || voiceActive) return
    await beginVoiceRamble()
  }

  async function beginVoiceRamble() {
    session.transition('starting', t($locale, 'Starting the microphone and live transcription…'))
    const voiceStarted = await voice.start($session.requestId)
    if (disposed) return
    if (!voiceStarted || !$voice.sessionId) {
      session.transition('error', $voice.message || t($locale, 'Microphone failed to start'))
      return
    }

    session.transition('active', t($locale, 'Ramble active · Clipboard is read only when you click import'))
  }

  async function stopRamble() {
    if (!rambleCanStop || $session.phase === 'stopping') return
    session.transition('stopping', t($locale, 'Finishing the final speech segment and pausing…'))
    let stopError = ''
    if (voiceCanStop) {
      const voiceStopped = await voice.stop()
      if (!voiceStopped && !stopError) stopError = $voice.message || t($locale, 'Microphone failed to stop')
    }
    if (stopError) {
      session.transition('error', stopError)
    } else {
      session.transition('paused', t($locale, 'Ramble paused; the document is preserved and capture tools remain available'))
    }
  }

  async function handleClipboardCaptureResult(
    result: Awaited<ReturnType<WorkbenchCapabilities['clipboardCapture']['implementation']['captureOnce']>>,
    target: AttachmentCandidateTarget,
  ) {
    if (disposed || interactionLocked || !target.requestId) {
      if (result.kind === 'attachment') await result.candidate.dispose().catch(() => {})
      return
    }

    const label = clipboardCaptureLabel(result.capturedAtMs, result.kind === 'text' && result.truncated, $locale)
    if (result.kind === 'text') {
      await onRouteDraftOperation(target.requestId, {
        kind: 'appendClipboardText', text: result.text, label, action: target.action,
      })
      if (!disposed && $session.requestId === target.requestId) {
        clipboardCaptureCount += 1
        session.setMessage(t($locale, 'Ramble active · {count} clipboard items captured', { count: clipboardCaptureCount }))
      }
      return
    }

    clipboardImageQueue = clipboardImageQueue
      .then(async () => {
        const persisted = await onPersistAttachmentCandidates(
          { ...target, label },
          [result.candidate],
        )
        if (!persisted) {
          clipboardFailure = t($locale, 'Clipboard import could not be completed. Review the attachment message before continuing.')
          return
        }
        if (disposed || $session.requestId !== target.requestId) return
        clipboardCaptureCount += 1
        session.setMessage(t($locale, 'Ramble active · {count} clipboard items captured', { count: clipboardCaptureCount }))
      })
      .catch((cause) => {
        clipboardFailure = t($locale, 'Could not insert clipboard image: {error}', { error: messageFrom(cause) })
        if (!disposed) onAttachmentMessage(clipboardFailure)
      })
    await clipboardImageQueue
  }

  function markRambleRecording() {
    if ($voice.phase === 'stopping' || $session.phase === 'stopping' || $session.phase === 'active') return
    session.transition('active', t($locale, 'Ramble active · Clipboard is read only when you click import'))
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
        if (exitFlight) { await exitFlight; break }
        await rambleTransition.run(async () => {
          if (interactionLocked || !$session.requestId) return
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
    if (!rambleEngaged || !$session.requestId) return
    const state: RambleConsoleState = {
      phase:
        visibleRamblePhase === 'active'
          ? 'recording'
          : visibleRamblePhase === 'idle'
            ? 'paused'
            : visibleRamblePhase,
      sourceLabel: rambleSourceLabel,
      requestTitle: $session.requestTitle,
      recording: visibleRamblePhase === 'active',
      busy: rambleBusy,
      captureBusy: screenCaptureBusy,
      voiceLevel: $voice.level,
      partialTranscript: $voice.partial,
      message: $session.message,
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

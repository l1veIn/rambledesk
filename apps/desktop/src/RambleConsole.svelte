<script lang="ts">
  import { emitTo, listen } from '@tauri-apps/api/event'
  import { getCurrentWebview } from '@tauri-apps/api/webview'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { open } from '@tauri-apps/plugin-dialog'
  import {
    ClipboardPaste,
    FilePlus2,
    GripVertical,
    LoaderCircle,
    LogOut,
    Mic,
    ScanLine,
    Send,
    MessageSquare,
    FileText,
  } from '@lucide/svelte'
  import { onMount } from 'svelte'

  import { t } from './lib/i18n'
  import { locale } from './lib/preferences'
  import { speechTargetLabel } from './lib/speech/speechTargetLabel'
  import {
    RAMBLE_CONSOLE_COMMAND_EVENT,
    RAMBLE_CONSOLE_READY_EVENT,
    RAMBLE_CONSOLE_HIDE_EVENT,
    RAMBLE_CONSOLE_SHOW_EVENT,
    RAMBLE_CONSOLE_STATE_EVENT,
    type RambleConsoleCommand,
    type RambleConsoleState,
  } from './lib/rambleConsole'

  let state: RambleConsoleState | null = null
  let dragActive = false
  let localBusy = false
  let errorMessage = ''
  const isTauri = '__TAURI_INTERNALS__' in window

  $: recording = state?.recording ?? false
  $: busy = localBusy || (state?.busy ?? true)
  $: statusLabel = !state
    ? t($locale, 'Waiting for the main window…')
    : state.phase === 'starting'
      ? t($locale, 'Starting…')
      : state.phase === 'recording'
        ? t($locale, 'Recording')
        : state.phase === 'stopping'
          ? t($locale, 'Pausing…')
          : state.phase === 'paused'
            ? t($locale, 'Ramble paused')
            : state.phase === 'error'
              ? state.message
              : t($locale, 'Ready')
  $: recordingLabel =
    state?.phase === 'starting'
      ? t($locale, 'Starting…')
      : state?.phase === 'stopping'
        ? t($locale, 'Pausing…')
        : recording
          ? t($locale, 'Recording')
          : t($locale, 'Resume recording')
  $: consoleMessage = errorMessage || state?.message || statusLabel
  $: target = state?.target ?? state?.nextTarget
  $: targetLabel = target ? speechTargetLabel(target, (source) => t($locale, source)) : ''
  $: nextTargetLabel = state?.nextTarget ? speechTargetLabel(state.nextTarget, (source) => t($locale, source)) : ''
  $: targetDescription = `${t($locale, 'Current segment')}: ${targetLabel}${nextTargetLabel ? `\n${t($locale, 'Next segment')}: ${nextTargetLabel}` : ''}`

  onMount(() => {
    if (!isTauri) return
    let stateUnlisten: (() => void) | undefined
    let dragUnlisten: (() => void) | undefined
    let showUnlisten: (() => void) | undefined
    let hideUnlisten: (() => void) | undefined

    void listen<RambleConsoleState>(RAMBLE_CONSOLE_STATE_EVENT, (event) => {
      state = event.payload
      errorMessage = ''
    }).then((unlisten) => {
      stateUnlisten = unlisten
      void emitTo('main', RAMBLE_CONSOLE_READY_EVENT)
    })

    void getCurrentWebview()
      .onDragDropEvent((event) => {
        dragActive = event.payload.type === 'enter' || event.payload.type === 'over'
        if (event.payload.type === 'drop') {
          dragActive = false
          void send({
            type: 'import-server-paths',
            serverPaths: event.payload.paths,
          })
        } else if (event.payload.type === 'leave') {
          dragActive = false
        }
      })
      .then((unlisten) => {
        dragUnlisten = unlisten
      })
      .catch((cause) => {
        errorMessage = String(cause)
      })
    void listen(RAMBLE_CONSOLE_SHOW_EVENT, () => {
      void getCurrentWindow().show()
    }).then((unlisten) => {
      showUnlisten = unlisten
    })
    void listen(RAMBLE_CONSOLE_HIDE_EVENT, () => {
      void getCurrentWindow().hide()
    }).then((unlisten) => {
      hideUnlisten = unlisten
    })

    return () => {
      stateUnlisten?.()
      dragUnlisten?.()
      showUnlisten?.()
      hideUnlisten?.()
    }
  })

  async function send(command: RambleConsoleCommand) {
    if (localBusy && command.type !== 'exit') return
    errorMessage = ''
    try {
      await emitTo('main', RAMBLE_CONSOLE_COMMAND_EVENT, command)
    } catch (cause) {
      errorMessage = cause instanceof Error ? cause.message : String(cause)
    }
  }

  async function chooseFiles() {
    localBusy = true
    try {
      const selected = await open({ multiple: true, directory: false })
      const paths = selected ? (Array.isArray(selected) ? selected : [selected]) : []
      if (paths.length > 0) {
        await send({ type: 'import-server-paths', serverPaths: paths })
      }
    } catch (cause) {
      errorMessage = cause instanceof Error ? cause.message : String(cause)
    } finally {
      localBusy = false
    }
  }

  async function startDragging(event: PointerEvent) {
    if (!isTauri || event.button !== 0) return
    const target = event.target
    if (target instanceof Element && target.closest('.console-tool')) return
    try {
      await getCurrentWindow().startDragging()
    } catch (cause) {
      errorMessage = cause instanceof Error ? cause.message : String(cause)
    }
  }
</script>

<div
  class:drop-active={dragActive}
  class:recording
  class="floating-console"
  role="toolbar"
  tabindex="0"
  aria-label={t($locale, 'Ramble console')}
  title={consoleMessage}
  onpointerdown={(event) => void startDragging(event)}
>
  <span
    class="console-grip"
    title={t($locale, 'Drag floating console')}
  >
    <GripVertical size={14} strokeWidth={1.8} aria-hidden="true" />
    {#if target}
      <button type="button" class="console-tool target-tool" disabled={target.destination.kind === 'unknown'} title={targetDescription} aria-label={targetDescription}
        onclick={() => target && send({ type: 'open-speech-target', requestId: target.requestId, target })}>
        {#if target.destination.kind === 'review_annotation' || target.destination.kind === 'question_answer'}<MessageSquare size={15} />{:else}<FileText size={15} />{/if}
      </button>
    {/if}
  </span>

  <span class="console-divider" aria-hidden="true"></span>

  <div class="floating-tools">
    <button
      class:active={recording}
      class="console-tool"
      disabled={busy}
      onclick={() => send({ type: 'toggle-recording' })}
      title={`${recordingLabel} · Ctrl + Shift + R`}
      aria-label={recordingLabel}
      aria-pressed={recording}
    >
      {#if state?.phase === 'starting' || state?.phase === 'stopping'}
        <LoaderCircle class="animate-spin" size={20} strokeWidth={1.8} />
      {:else if recording}
        <span class="record-dot record-blink" aria-hidden="true"></span>
        <Mic size={20} strokeWidth={1.8} />
      {:else}
        <Mic size={20} strokeWidth={1.8} />
      {/if}
      <span class="voice-level" style={`--level:${Math.max(0.06, state?.voiceLevel ?? 0)}`}></span>
    </button>
    <button
      class="console-tool"
      disabled={state?.captureBusy || !state}
      onclick={() => send({ type: 'capture-screen' })}
      title={`${t($locale, 'Add screenshot to selected input')} · Ctrl + Shift + 1`}
      aria-label={t($locale, 'Add screenshot to selected input')}
    >
      {#if state?.captureBusy}
        <LoaderCircle class="animate-spin" size={20} strokeWidth={1.75} />
      {:else}
        <ScanLine size={20} strokeWidth={1.75} />
      {/if}
    </button>
    <button
      class="console-tool"
      disabled={state?.captureBusy || !state}
      onclick={() => send({ type: 'import-clipboard' })}
      title={t($locale, 'Paste text or images into the selected input')}
      aria-label={t($locale, 'Paste text or images into the selected input')}
    >
      <ClipboardPaste size={20} strokeWidth={1.75} />
    </button>
    <button
      class="console-tool"
      disabled={state?.captureBusy || !state || localBusy}
      onclick={chooseFiles}
      title={t($locale, 'Add files to selected input')}
      aria-label={t($locale, 'Add files to selected input')}
    >
      <FilePlus2 size={20} strokeWidth={1.75} />
    </button>
    <button
      class="console-tool submit-tool"
      disabled={busy || !state?.canSubmit}
      onclick={() => send({ type: 'submit' })}
      title={t($locale, 'Submit feedback')}
      aria-label={t($locale, 'Submit feedback')}
    >
      <Send size={20} strokeWidth={1.75} />
    </button>
    <button
      class="console-tool exit-tool"
      onclick={() => send({ type: 'exit' })}
      title={t($locale, 'Exit Ramble')}
      aria-label={t($locale, 'Exit Ramble')}
    >
      <LogOut size={20} strokeWidth={1.75} />
    </button>
  </div>

  {#if dragActive}
    <div class="drop-prompt" title={t($locale, 'Add files to selected input')}>
      <FilePlus2 size={23} strokeWidth={1.8} />
    </div>
  {/if}
</div>

<style>
  .console-grip { display: flex; justify-content: center; gap: 2px; }
  .target-tool { display: grid; place-items: center; width: 24px; height: 24px; padding: 0; border: 0; border-radius: 6px; color: var(--muted-foreground); background: transparent; cursor: pointer; }
  .target-tool:hover { color: var(--primary); background: var(--muted); }
  .target-tool:focus-visible { outline: 2px solid var(--ring); outline-offset: 1px; }
</style>

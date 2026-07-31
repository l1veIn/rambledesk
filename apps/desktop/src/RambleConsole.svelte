<script lang="ts">
  import { emitTo, listen } from '@tauri-apps/api/event'
  import { getCurrentWebview } from '@tauri-apps/api/webview'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { open } from '@tauri-apps/plugin-dialog'
  import { onMount } from 'svelte'

  import { t } from './lib/i18n'
  import { locale } from './lib/preferences'
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

  $: recording = state?.recording ?? false
  $: busy = localBusy || (state?.busy ?? true)
  $: statusLabel = !state
    ? t($locale, '等待主窗口…')
    : state.phase === 'recording'
      ? t($locale, '正在记录')
      : state.phase === 'paused'
        ? t($locale, 'Ramble 已暂停')
        : state.phase === 'error'
          ? state.message
          : t($locale, '准备就绪')

  onMount(() => {
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
          void send({ type: 'import-files', paths: event.payload.paths })
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
      if (paths.length > 0) await send({ type: 'import-files', paths })
    } catch (cause) {
      errorMessage = cause instanceof Error ? cause.message : String(cause)
    } finally {
      localBusy = false
    }
  }
</script>

<main class:drop-active={dragActive} class="floating-console">
  <header class="floating-heading" data-tauri-drag-region>
    <div class="floating-brand" data-tauri-drag-region>
      <svg viewBox="0 0 32 32" aria-hidden="true" data-tauri-drag-region>
        <path d="M16 2.8 27 9.2v13.6L16 29.2 5 22.8V9.2Z" />
        <path d="m16 7.7 6.8 4v8.6L16 24.3l-6.8-4v-8.6Z" />
      </svg>
      <div data-tauri-drag-region>
        <strong data-tauri-drag-region>RAMBLE</strong>
        <span data-tauri-drag-region>{state?.projectName ?? 'RambleDesk'}</span>
      </div>
    </div>
    <div class:recording class="floating-status" data-tauri-drag-region>
      <i></i>
      <span data-tauri-drag-region>{statusLabel}</span>
    </div>
    <span class="drag-handle" title={t($locale, '拖动悬浮窗')} data-tauri-drag-region>⠿</span>
  </header>

  <section class="floating-body">
    <div class="recording-summary">
      <div class="level-ring" style={`--level:${Math.max(0.08, state?.voiceLevel ?? 0)}`}>
        <span>{recording ? 'Ⅱ' : '●'}</span>
      </div>
      <div>
        <strong>{state?.requestTitle ?? statusLabel}</strong>
        <span>
          {#if state?.partialTranscript}
            {t($locale, '正在听：{text}', { text: state.partialTranscript })}
          {:else}
            {errorMessage || state?.message || t($locale, '拖入文件即可写入当前文档')}
          {/if}
        </span>
      </div>
    </div>

    <div class="floating-tools" aria-label="Ramble tools">
      <button
        class:active={recording}
        disabled={busy}
        onclick={() => send({ type: 'toggle-recording' })}
        title="Ctrl + Shift + R"
      >
        <span>{recording ? 'Ⅱ' : '●'}</span>
        {recording ? t($locale, '暂停录音') : t($locale, '继续录音')}
      </button>
      <button disabled={state?.captureBusy || !state} onclick={() => send({ type: 'capture-screen' })} title="Ctrl + Shift + 1">
        <span>⌗</span>{t($locale, '截图')}
      </button>
      <button disabled={state?.captureBusy || !state} onclick={() => send({ type: 'import-clipboard' })}>
        <span>▣</span>{t($locale, '剪贴板')}
      </button>
      <button disabled={state?.captureBusy || !state || localBusy} onclick={chooseFiles} title={t($locale, '选择文件')}>
        <span>＋</span>{t($locale, '文件')}
      </button>
      <button class="exit-tool" onclick={() => send({ type: 'exit' })}>
        <span>×</span>{t($locale, '退出 Ramble')}
      </button>
    </div>
  </section>

  {#if dragActive}
    <div class="drop-prompt"><span>＋</span>{t($locale, '拖入文件即可写入当前文档')}</div>
  {/if}
</main>

<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { emitTo, listen } from '@tauri-apps/api/event'
  import { onMount, tick } from 'svelte'

  import {
    clipboardCaptureLabel,
    eventBelongsToRamble,
    type ClipboardCaptureEvent,
  } from '../clipboardCapture'
  import { attachmentMarkdownUrl } from '../attachmentMarkdown'
  import type { AddAttachmentInput, FeedbackWorkspaceView } from '../feedback'
  import { t } from '../i18n'
  import { locale } from '../preferences'
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
    type SpeechEvent,
    type VoiceRambleSessionView,
  } from '../speech'
  import type { FeedbackEditorHandle, RamblePhase, VoicePhase } from './types'

  export let isTauri = false
  export let workspace: FeedbackWorkspaceView | null = null
  export let editor: FeedbackEditorHandle | undefined
  export let attachmentBusy = false
  export let attachmentMessage = ''
  export let voicePhase: VoicePhase = 'idle'
  export let voiceDevice = ''
  export let voicePartial = ''
  export let voiceLevel = 0
  export let voiceChunkIndex = 0
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
  export let onAppendRambleMarkdown: (requestId: string, markdown: string) => Promise<void> = async () => {}

  let voiceRequestId = ''
  let voiceSessionId = ''
  let rambleContextId = ''
  let rambleSourceLabel = ''
  let clipboardCaptureCount = 0
  let clipboardImageQueue: Promise<void> = Promise.resolve()

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
  $: if (rambleEngaged && workspace) broadcastRambleConsoleState()

  onMount(() => {
    if (!isTauri) return
    let voiceUnlisten: (() => void) | undefined
    let rambleShortcutUnlisten: (() => void) | undefined
    let captureShortcutUnlisten: (() => void) | undefined
    let consoleCommandUnlisten: (() => void) | undefined
    let consoleReadyUnlisten: (() => void) | undefined

    void listen<SpeechEvent>('voice-ramble-event', (event) => {
      handleVoiceEvent(event.payload)
    })
      .then((unlisten) => {
        voiceUnlisten = unlisten
      })
      .catch((cause) => {
        voicePhase = 'error'
        voiceMessage = t($locale, '无法监听语音识别事件：{error}', { error: messageFrom(cause) })
      })
    void listen<string>('screen-capture-shortcut', () => {
      if (rambleEngaged) void onStartScreenCapture()
    })
      .then((unlisten) => {
        captureShortcutUnlisten = unlisten
      })
      .catch((cause) => {
        attachmentMessage = t($locale, '无法监听截图快捷键：{error}', { error: messageFrom(cause) })
      })
    void listen<string>('ramble-toggle-shortcut', () => {
      void toggleRamble()
    })
      .then((unlisten) => {
        rambleShortcutUnlisten = unlisten
      })
      .catch((cause) => {
        ramblePhase = 'error'
        rambleMessage = t($locale, '无法监听 Ramble 快捷键：{error}', { error: messageFrom(cause) })
      })
    void listen<RambleConsoleCommand>(RAMBLE_CONSOLE_COMMAND_EVENT, (event) => {
      void handleRambleConsoleCommand(event.payload)
    }).then((unlisten) => {
      consoleCommandUnlisten = unlisten
    })
    void listen(RAMBLE_CONSOLE_READY_EVENT, () => {
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
      if (voiceCanStop) void invoke('stop_voice_ramble')
    }
  })

  export async function toggleRamble() {
    if (rambleBusy) return
    if (rambleActive || voiceCanStop) await stopRamble()
    else if (rambleEngaged) await resumeRamble()
    else await startRamble()
  }

  export async function exitRamble() {
    if (!rambleCanExit && !rambleStartedOnce) return
    if (voiceCanStop) {
      ramblePhase = 'stopping'
      rambleMessage = t($locale, '正在结束 Ramble…')
      await stopVoiceRamble()
    }
    void emitTo('ramble-console', RAMBLE_CONSOLE_HIDE_EVENT).catch(() => {})
    resetVoiceUi()
    resetRambleUi()
  }

  export async function importClipboardNow() {
    if (!rambleRequestId || !rambleEngaged || !rambleContextId || attachmentBusy) return
    attachmentMessage = ''
    try {
      const event = await invoke<ClipboardCaptureEvent>('capture_clipboard_once', {
        input: {
          request_id: rambleRequestId,
          ramble_context_id: rambleContextId,
        },
      })
      handleClipboardCaptureEvent(event)
    } catch (cause) {
      attachmentMessage = t($locale, '无法导入剪贴板：{error}', { error: messageFrom(cause) })
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
    rambleMessage = t($locale, '正在打开 Ramble 操作台…')
    void emitTo('ramble-console', RAMBLE_CONSOLE_SHOW_EVENT).catch((cause) => {
      onPageError(t($locale, '无法打开 Ramble 操作台：{error}', { error: messageFrom(cause) }))
    })
    await resumeRamble()
  }

  async function resumeRamble() {
    if (!rambleRequestId || rambleBusy || rambleActive || !rambleContextId) return
    ramblePhase = 'starting'
    rambleMessage = t($locale, '正在启动麦克风与实时转写…')
    const voiceStarted = await startVoiceRamble()
    if (!voiceStarted || !voiceSessionId) {
      ramblePhase = 'error'
      rambleMessage = voiceMessage || t($locale, '麦克风启动失败')
      return
    }

    ramblePhase = 'active'
    rambleMessage = t($locale, 'Ramble 进行中 · 剪贴板仅在点击导入时读取')
  }

  async function stopRamble() {
    if (!rambleCanStop || ramblePhase === 'stopping') return
    ramblePhase = 'stopping'
    rambleMessage = t($locale, '正在收尾最后一段语音并暂停记录…')
    let stopError = ''
    if (voiceCanStop) {
      const voiceStopped = await stopVoiceRamble()
      if (!voiceStopped && !stopError) stopError = voiceMessage || t($locale, '麦克风停止失败')
    }
    if (stopError) {
      ramblePhase = 'error'
      rambleMessage = stopError
    } else {
      ramblePhase = 'paused'
      rambleMessage = t($locale, 'Ramble 已暂停；正文保留，截图和导入仍可使用')
    }
  }

  async function startVoiceRamble(): Promise<boolean> {
    if (!rambleRequestId || voiceActive) return false
    voicePhase = 'starting'
    voiceRequestId = rambleRequestId
    voiceSessionId = ''
    voiceDevice = ''
    voicePartial = ''
    voiceMessage = t($locale, '正在加载本地模型并连接麦克风…')
    voiceLevel = 0
    try {
      const session = await invoke<VoiceRambleSessionView>('start_voice_ramble', {
        input: {
          request_id: rambleRequestId,
        },
      })
      voiceSessionId = session.voice_session_id
      if (voicePhase === 'starting') {
        voicePhase = 'listening'
        voiceMessage = t($locale, 'Sherpa 真流式识别 · 自然停顿后写入正文')
      }
    } catch (cause) {
      voicePhase = 'error'
      voiceMessage = messageFrom(cause)
      return false
    }
    return true
  }

  async function stopVoiceRamble(): Promise<boolean> {
    if (!voiceCanStop) return true
    voicePhase = 'stopping'
    voiceMessage = t($locale, '正在完成最后一段识别…')
    try {
      await invoke('stop_voice_ramble')
      for (let attempt = 0; attempt < 5 && voicePhase === 'stopping'; attempt += 1) {
        await new Promise((resolve) => setTimeout(resolve, 20))
      }
      await tick()
      if (voicePhase === 'stopping') {
        voicePhase = 'idle'
        voiceMessage = t($locale, '录音已停止')
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

  function handleClipboardCaptureEvent(event: ClipboardCaptureEvent) {
    if (
      !rambleEngaged ||
      !rambleRequestId ||
      !eventBelongsToRamble(
        event,
        rambleRequestId,
        rambleContextId,
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
      if (workspace?.request.request_id === rambleRequestId) {
        editor?.appendClipboardCapture(event.text, label)
      } else {
        const quoted = event.text.split(/\r?\n/).map((line) => `> ${line}`).join('\n')
        void onAppendRambleMarkdown(rambleRequestId, `> **${label}**\n>\n${quoted}`).catch(
          (cause) => onPageError(t($locale, 'Ramble 内容写入失败：{error}', { error: messageFrom(cause) })),
        )
      }
      clipboardCaptureCount += 1
      rambleMessage = t($locale, 'Ramble 进行中 · 已捕获 {count} 项剪贴板上下文', { count: clipboardCaptureCount })
      return
    }

    clipboardImageQueue = clipboardImageQueue
      .then(() => importClipboardImage(event))
      .catch((cause) => {
        attachmentMessage = t($locale, '剪贴板图片写入失败：{error}', { error: messageFrom(cause) })
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
      if (attachmentBusy) throw new Error(t($locale, '附件通道正忙，请稍后重新复制图片'))
      const visibleTarget = workspace?.request.request_id === requestId
      if (visibleTarget && !(await onSaveDraftNow())) {
        throw new Error(t($locale, '当前草稿无法保存'))
      }
      const target = visibleTarget
        ? workspace
        : await invoke<FeedbackWorkspaceView>('get_feedback_workspace', { requestId })
      if (!target) return

      attachmentBusy = true
      const png = await invoke<ArrayBuffer>('read_clipboard_capture_image', {
        captureId: event.capture_id,
        requestId,
        rambleContextId: event.ramble_context_id,
      })
      const input: AddAttachmentInput = {
        request_id: requestId,
        file_name: event.file_name,
        contents: Array.from(new Uint8Array(png)),
        expected_revision: target.draft.saved_revision,
      }
      const next = await invoke<FeedbackWorkspaceView>('add_feedback_attachment', { input })
      const attachment = next.attachments.find(
        (item) => !target.attachments.some(
          (existing) => existing.attachment_id === item.attachment_id,
        ),
      )
      if (!attachment) throw new Error(t($locale, '图片附件已保存，但未能写入文档流'))
      const label = clipboardCaptureLabel(event.captured_at_ms, false, $locale)
      if (visibleTarget && workspace?.request.request_id === requestId) {
        onApplyWorkspaceMutation(next)
        await onRefreshAttachmentPreviews(next)
        await tick()
        if (!editor?.appendCapturedAttachment(attachment, label)) {
          throw new Error(t($locale, '图片附件已保存，但未能写入文档流'))
        }
        await onSaveDraftNow()
      } else {
        await onAppendRambleMarkdown(
          requestId,
          `> **${label}**\n\n![${attachment.file_name}](${attachmentMarkdownUrl(attachment.attachment_id)})`,
        )
      }
      clipboardCaptureCount += 1
      rambleMessage = t($locale, 'Ramble 进行中 · 已捕获 {count} 项剪贴板上下文', { count: clipboardCaptureCount })
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
        voiceMessage = t($locale, '正在录音 · {device}', { device: event.input_device })
        break
      case 'partial':
        voicePartial = event.text
        if (voicePhase !== 'stopping') voicePhase = 'listening'
        break
      case 'level':
        voiceLevel = Math.min(1, Math.max(0, event.rms * 8))
        if (voicePhase !== 'stopping') voicePhase = 'listening'
        break
      case 'processing':
        voiceChunkIndex = event.chunk_index + 1
        if (voicePhase !== 'stopping') voicePhase = 'processing'
        voiceMessage = t($locale, '正在识别第 {count} 段…', { count: event.chunk_index + 1 })
        break
      case 'stable': {
        const transcript = stableTranscript(event)
        if (transcript) {
          if (workspace?.request.request_id === voiceRequestId) editor?.appendTranscript(transcript)
          else {
            void onAppendRambleMarkdown(voiceRequestId, transcript).catch(
              (cause) => onPageError(t($locale, 'Ramble 内容写入失败：{error}', { error: messageFrom(cause) })),
            )
          }
        }
        voicePartial = ''
        voiceChunkIndex = event.chunk_index + 1
        if (voicePhase !== 'stopping') voicePhase = 'listening'
        voiceMessage = t($locale, '第 {count} 段已写入正文', { count: event.chunk_index + 1 })
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
        voiceMessage = t($locale, '录音已停止')
        if (ramblePhase === 'active') {
          ramblePhase = 'error'
          rambleMessage = t($locale, '麦克风意外停止，Ramble 已暂停')
        }
        break
      case 'error':
        voicePhase = 'error'
        voiceLevel = 0
        voicePartial = ''
        voiceMessage = event.message
        if (ramblePhase === 'active') {
          ramblePhase = 'error'
          rambleMessage = t($locale, '麦克风错误，Ramble 已暂停：{error}', { error: event.message })
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
        await onStartScreenCapture()
        break
      case 'import-clipboard':
        await importClipboardNow()
        break
      case 'import-files':
        await onImportAttachmentPaths(command.paths)
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
      captureBusy: attachmentBusy,
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

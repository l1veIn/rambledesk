<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { Check, Camera, GripHorizontal, X } from '@lucide/svelte'
  import { onMount } from 'svelte'

  type ScrollCaptureInfo = {
    session_id: string
    frame_count: number
    width: number
    height: number
    added_height: number
    matched: boolean
  }

  let info: ScrollCaptureInfo | null = null
  let busy = false
  let errorMessage = ''
  let statusMessage = '滚动目标内容，RambleDesk 会自动连续采集'

  onMount(() => {
    const keydown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') void cancel()
      if (event.key === ' ' && !busy) {
        event.preventDefault()
        void captureFrame()
      }
      if (event.key === 'Enter' && !busy) void finish()
    }
    window.addEventListener('keydown', keydown)
    void initialize()
    const automaticCapture = window.setInterval(() => {
      if (!busy && info) void captureFrame()
    }, 240)
    return () => {
      window.clearInterval(automaticCapture)
      window.removeEventListener('keydown', keydown)
    }
  })

  async function initialize() {
    try {
      info = await invoke<ScrollCaptureInfo>('get_scrolling_capture_info')
    } catch (cause) {
      errorMessage = messageFrom(cause)
    }
  }

  async function captureFrame() {
    if (!info || busy) return
    busy = true
    errorMessage = ''
    statusMessage = '正在匹配滚动位置…'
    try {
      info = await invoke<ScrollCaptureInfo>('append_scrolling_capture_frame', {
        sessionId: info.session_id,
      })
      statusMessage = info.matched
        ? info.added_height > 0
          ? `已拼接 ${info.added_height} px 新内容`
          : '画面没有变化，请继续滚动'
        : '没有找到可靠重叠区域，请少滚动一些再试'
    } catch (cause) {
      errorMessage = messageFrom(cause)
      statusMessage = '这一帧没有写入长图'
    } finally {
      busy = false
    }
  }

  async function finish() {
    if (!info || busy) return
    busy = true
    errorMessage = ''
    statusMessage = '正在生成长图并返回标注编辑器…'
    try {
      await invoke('finish_scrolling_capture', { sessionId: info.session_id })
    } catch (cause) {
      errorMessage = messageFrom(cause)
      busy = false
    }
  }

  async function cancel() {
    if (busy) return
    busy = true
    try {
      await invoke('cancel_screen_capture')
    } catch (cause) {
      errorMessage = messageFrom(cause)
      busy = false
    }
  }

  async function startDragging(event: PointerEvent) {
    if (event.button !== 0) return
    const target = event.target
    if (target instanceof Element && target.closest('button')) return
    await getCurrentWindow().startDragging().catch(() => {})
  }

  function messageFrom(cause: unknown) {
    return cause instanceof Error ? cause.message : String(cause)
  }
</script>

<main class="scroll-controller" onpointerdown={(event) => void startDragging(event)}>
  <GripHorizontal class="grip" size={18} strokeWidth={1.8} />
  <section>
    <strong>滚动截图</strong>
    <span>{statusMessage}</span>
    {#if info}
      <small>{info.frame_count} 帧 · {info.width} × {info.height}</small>
    {/if}
    {#if errorMessage}<em>{errorMessage}</em>{/if}
  </section>
  <div class="actions">
    <button disabled={!info || busy} onclick={captureFrame} title="立即采集一帧 · Space">
      <Camera size={17} />
      <span>立即采集</span>
    </button>
    <button class="finish" disabled={!info || busy} onclick={finish} title="完成 · Enter">
      <Check size={18} />
    </button>
    <button class="cancel" disabled={busy} onclick={cancel} title="取消 · Esc">
      <X size={18} />
    </button>
  </div>
</main>

<style>
  .scroll-controller {
    display: grid;
    width: 100%;
    height: 100%;
    grid-template-columns: 20px minmax(0, 1fr) auto;
    gap: 10px;
    align-items: center;
    padding: 12px;
    border: 1px solid rgb(255 255 255 / 14%);
    border-radius: 14px;
    color: #eff7ff;
    background: rgb(18 25 34 / 96%);
    box-shadow: 0 16px 44px rgb(0 0 0 / 34%);
    user-select: none;
  }

  :global(.grip) { color: rgb(255 255 255 / 36%); }
  section { display: grid; min-width: 0; gap: 2px; }
  strong { font-size: 12px; }
  span, small { overflow: hidden; color: rgb(239 247 255 / 66%); font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
  small { color: #72c8ff; }
  em { color: #ff9da5; font-size: 8px; font-style: normal; }
  .actions { display: flex; gap: 6px; }
  button {
    display: flex;
    height: 34px;
    align-items: center;
    justify-content: center;
    gap: 5px;
    padding: 0 9px;
    border: 1px solid rgb(255 255 255 / 12%);
    border-radius: 9px;
    color: #eff7ff;
    background: rgb(255 255 255 / 7%);
    cursor: pointer;
  }
  button span { color: inherit; font-size: 9px; }
  button:hover:not(:disabled) { background: rgb(255 255 255 / 13%); }
  button:disabled { cursor: wait; opacity: 0.42; }
  .finish { border-color: rgb(88 207 139 / 35%); background: rgb(50 150 94 / 30%); }
  .cancel { color: #ffb5bb; }
</style>

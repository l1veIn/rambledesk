<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { onMount } from 'svelte'

  import { t } from './lib/i18n'
  import { locale } from './lib/preferences'
  import {
    normalizeCaptureSelection,
    type CapturePoint,
    type ScreenCaptureView,
  } from './lib/screenCapture'

  let capture: ScreenCaptureView | null = null
  let previewUrl = ''
  let start: CapturePoint | null = null
  let current: CapturePoint | null = null
  let completing = false
  let errorMessage = ''

  $: selection =
    start && current
      ? {
          left: Math.min(start.x, current.x),
          top: Math.min(start.y, current.y),
          width: Math.abs(current.x - start.x),
          height: Math.abs(current.y - start.y),
        }
      : null

  onMount(() => {
    const keydown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') void cancel()
    }
    window.addEventListener('keydown', keydown)
    void loadCapture()
    return () => {
      window.removeEventListener('keydown', keydown)
      if (previewUrl) URL.revokeObjectURL(previewUrl)
    }
  })

  async function loadCapture() {
    try {
      capture = await invoke<ScreenCaptureView>('get_screen_capture_view')
      const png = await invoke<ArrayBuffer>('read_screen_capture_preview', {
        sessionId: capture.session_id,
      })
      previewUrl = URL.createObjectURL(new Blob([png], { type: 'image/png' }))
    } catch (cause) {
      errorMessage = String(cause)
    }
  }

  function beginSelection(event: PointerEvent) {
    if (!capture || !previewUrl || completing || event.button !== 0) return
    const target = event.currentTarget as HTMLElement
    target.setPointerCapture(event.pointerId)
    start = { x: event.clientX, y: event.clientY }
    current = start
  }

  function updateSelection(event: PointerEvent) {
    if (!start || completing) return
    current = { x: event.clientX, y: event.clientY }
  }

  async function finishSelection(event: PointerEvent) {
    if (!capture || !start || !current || completing) return
    current = { x: event.clientX, y: event.clientY }
    const rectangle = normalizeCaptureSelection(
      start,
      current,
      window.innerWidth,
      window.innerHeight,
      capture.width,
      capture.height,
    )
    if (rectangle.width < 4 || rectangle.height < 4) {
      start = null
      current = null
      return
    }
    completing = true
    try {
      await invoke('complete_screen_capture', {
        input: {
          session_id: capture.session_id,
          ...rectangle,
        },
      })
    } catch (cause) {
      completing = false
      errorMessage = String(cause)
      start = null
      current = null
    }
  }

  async function cancel() {
    if (completing) return
    completing = true
    try {
      await invoke('cancel_screen_capture')
    } catch (cause) {
      completing = false
      errorMessage = String(cause)
    }
  }
</script>

<main
  class="screenshot-overlay"
  class:ready={previewUrl}
  onpointerdown={beginSelection}
  onpointermove={updateSelection}
  onpointerup={finishSelection}
  oncontextmenu={(event) => {
    event.preventDefault()
    void cancel()
  }}
>
  {#if previewUrl}
    <img alt="" draggable="false" src={previewUrl} />
  {/if}

  <div class="capture-help">
      <strong>{completing ? t($locale, '正在写入文档…') : t($locale, '拖动鼠标框选截图区域')}</strong>
      <span>{t($locale, 'Esc 或右键取消 · 完成后自动插入 RambleDesk 文档流')}</span>
  </div>

  {#if selection}
    <div
      class="capture-selection"
      style={`left:${selection.left}px;top:${selection.top}px;width:${selection.width}px;height:${selection.height}px`}
    >
      {#if selection.width > 90 && selection.height > 34}
        <span>{Math.round(selection.width)} × {Math.round(selection.height)}</span>
      {/if}
    </div>
  {/if}

  {#if errorMessage}
    <div class="capture-error">
      <strong>{t($locale, '截图工具遇到问题')}</strong>
      <span>{errorMessage}</span>
      <button onclick={cancel}>{t($locale, '关闭')}</button>
    </div>
  {/if}
</main>

<style>
  .screenshot-overlay {
    position: fixed;
    inset: 0;
    overflow: hidden;
    color: white;
    background: #111;
    cursor: crosshair;
    user-select: none;
  }

  .screenshot-overlay img {
    width: 100%;
    height: 100%;
    pointer-events: none;
  }

  .capture-help {
    position: fixed;
    top: 22px;
    left: 50%;
    z-index: 4;
    display: flex;
    flex-direction: column;
    gap: 3px;
    align-items: center;
    padding: 10px 18px;
    border: 1px solid rgb(255 255 255 / 24%);
    border-radius: 12px;
    background: rgb(17 24 18 / 84%);
    box-shadow: 0 12px 36px rgb(0 0 0 / 28%);
    backdrop-filter: blur(12px);
    transform: translateX(-50%);
  }

  .capture-help strong {
    font-size: 14px;
  }

  .capture-help span {
    color: rgb(255 255 255 / 68%);
    font-size: 11px;
  }

  .capture-selection {
    position: fixed;
    z-index: 3;
    border: 2px solid #9ce68a;
    background: transparent;
    box-shadow: 0 0 0 9999px rgb(5 10 6 / 48%);
    pointer-events: none;
  }

  .capture-selection span {
    position: absolute;
    right: -2px;
    bottom: -28px;
    padding: 4px 7px;
    border-radius: 5px;
    color: white;
    background: #183b24;
    font-size: 11px;
  }

  .capture-error {
    position: fixed;
    top: 50%;
    left: 50%;
    z-index: 5;
    display: flex;
    width: min(420px, calc(100vw - 48px));
    flex-direction: column;
    gap: 10px;
    padding: 20px;
    border-radius: 14px;
    background: #fff;
    color: #3e1717;
    box-shadow: 0 20px 60px rgb(0 0 0 / 40%);
    cursor: default;
    transform: translate(-50%, -50%);
  }

  .capture-error button {
    align-self: flex-end;
    padding: 7px 14px;
    border: 0;
    border-radius: 8px;
    background: #254f33;
    color: white;
  }
</style>

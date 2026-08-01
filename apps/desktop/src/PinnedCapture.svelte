<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { X } from '@lucide/svelte'
  import { onMount } from 'svelte'

  const pinId = new URLSearchParams(window.location.hash.slice(1).replace('capture-pin=', 'pin_id=')).get('pin_id') ?? ''
  let imageUrl = ''
  let errorMessage = ''
  let closing = false

  onMount(() => {
    void loadImage()
    const keydown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') void closePin()
    }
    window.addEventListener('keydown', keydown)
    return () => {
      window.removeEventListener('keydown', keydown)
      if (imageUrl) URL.revokeObjectURL(imageUrl)
    }
  })

  async function loadImage() {
    try {
      const png = await invoke<ArrayBuffer>('read_pinned_screen_capture', { pinId })
      imageUrl = URL.createObjectURL(new Blob([png], { type: 'image/png' }))
    } catch (cause) {
      errorMessage = cause instanceof Error ? cause.message : String(cause)
    }
  }

  async function closePin() {
    if (closing) return
    closing = true
    await invoke('close_pinned_screen_capture', { pinId }).catch(() => {
      closing = false
    })
  }

  async function drag(event: PointerEvent) {
    if (event.button !== 0) return
    const target = event.target
    if (target instanceof Element && target.closest('button')) return
    await getCurrentWindow().startDragging().catch(() => {})
  }
</script>

<main
  class="pinned-capture"
  onpointerdown={(event) => void drag(event)}
  oncontextmenu={(event) => {
    event.preventDefault()
    void closePin()
  }}
>
  {#if imageUrl}
    <img src={imageUrl} alt="固定截图" draggable="false" />
  {:else if errorMessage}
    <span>{errorMessage}</span>
  {/if}
  <button onclick={closePin} title="关闭固定截图 · Esc" aria-label="关闭固定截图">
    <X size={16} strokeWidth={2} />
  </button>
</main>

<style>
  .pinned-capture {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    border: 1px solid rgb(255 255 255 / 32%);
    border-radius: 10px;
    background: #15191e;
    box-shadow: 0 14px 42px rgb(0 0 0 / 34%);
    user-select: none;
  }
  img { display: block; width: 100%; height: 100%; object-fit: contain; pointer-events: none; }
  span { display: grid; height: 100%; place-items: center; padding: 18px; color: #ffb0b6; font-size: 11px; }
  button {
    position: absolute;
    top: 7px;
    right: 7px;
    display: grid;
    width: 28px;
    height: 28px;
    place-items: center;
    padding: 0;
    border: 1px solid rgb(255 255 255 / 14%);
    border-radius: 8px;
    color: white;
    background: rgb(15 20 26 / 72%);
    opacity: 0;
    cursor: pointer;
    transition: opacity 120ms ease;
  }
  main:hover button, button:focus-visible { opacity: 1; }
</style>

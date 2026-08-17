<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { AlertCircle, ExternalLink, FileQuestion, LoaderCircle, Minus, Plus, RotateCcw } from '@lucide/svelte'
  import { onDestroy, tick } from 'svelte'

  import { Button } from '$lib/components/ui/button'
  import * as Dialog from '$lib/components/ui/dialog'
  import type { AttachmentView, RequestAttachmentView } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import MarkdownPreview from './MarkdownPreview.svelte'

  export let open = false
  export let requestId = ''
  export let readKind: 'request' | 'workspace' = 'request'
  export let attachment: (RequestAttachmentView & AttachmentView) | null = null

  let loading = false
  let error = ''
  let markdown = ''
  let imageUrl = ''
  let imageElement: HTMLImageElement
  let imageContainer: HTMLDivElement
  let scale = 1
  let offsetX = 0
  let offsetY = 0
  let dragging = false
  let dragStartX = 0
  let dragStartY = 0
  let dragOriginX = 0
  let dragOriginY = 0
  let unsupported = false
  let openMessage = ''
  let openError = ''
  let loadedKey = ''
  let loadGeneration = 0

  $: requestedKey = open && attachment ? `${requestId}:${attachment.attachment_id}` : ''
  $: if (requestedKey && requestedKey !== loadedKey) void loadAttachment(requestedKey)
  $: if (!open && loadedKey) resetPreview()

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  async function loadAttachment(key: string) {
    const current = attachment
    if (!current || !requestId) return
    const generation = ++loadGeneration
    releaseMedia()
    loadedKey = key
    loading = true
    error = ''
    markdown = ''
    unsupported = false
    openMessage = ''
    openError = ''
    try {
      const raw =
        readKind === 'workspace'
          ? await invoke<ArrayBuffer>('read_feedback_attachment', {
              requestId,
              attachmentId: current.attachment_id,
            })
          : await invoke<ArrayBuffer>('read_request_attachment', {
              requestId,
              attachmentId: current.attachment_id,
            })
      if (generation !== loadGeneration || !open) return
      const buffer =
        raw instanceof ArrayBuffer ? new Uint8Array(raw) : Uint8Array.from(raw)
      if (current.media_type === 'text/markdown') {
        markdown = new TextDecoder('utf-8', { fatal: true }).decode(buffer)
      } else if (current.media_type.startsWith('image/')) {
        imageUrl = URL.createObjectURL(new Blob([buffer], { type: current.media_type }))
        await tick()
        if (generation !== loadGeneration || !imageElement) return
        await imageElement.decode()
        if (generation !== loadGeneration || !open) return
      } else {
        unsupported = true
      }
    } catch (cause) {
      if (generation !== loadGeneration) return
      error = messageFrom(cause)
    } finally {
      if (generation === loadGeneration) loading = false
    }
  }

  function messageFrom(cause: unknown) {
    if (cause instanceof Error) return cause.message
    if (typeof cause === 'string') return cause
    if (cause && typeof cause === 'object' && 'message' in cause) {
      return String((cause as { message: unknown }).message)
    }
    return tr('An unknown error occurred while reading the attachment.')
  }

  function resetPreview() {
    loadGeneration += 1
    loadedKey = ''
    loading = false
    error = ''
    markdown = ''
    unsupported = false
    openMessage = ''
    openError = ''
    releaseMedia()
  }

  async function openExternally() {
    const current = attachment
    if (!current || !requestId) return
    openError = ''
    openMessage = ''
    try {
      const path = await invoke<string>('open_feedback_attachment', {
        input: {
          requestId,
          attachmentId: current.attachment_id,
          kind: readKind,
        },
      })
      openMessage = tr('Opened in the system default app: {path}', { path })
    } catch (cause) {
      openError = messageFrom(cause)
    }
  }

  const MIN_SCALE = 1
  const MAX_SCALE = 8

  function clampScale(value: number) {
    return Math.min(MAX_SCALE, Math.max(MIN_SCALE, value))
  }

  function resetZoom() {
    scale = 1
    offsetX = 0
    offsetY = 0
  }

  function applyZoom(nextScale: number, anchorX: number, anchorY: number) {
    const clamped = clampScale(nextScale)
    if (clamped === scale) return
    const ratio = clamped / scale
    offsetX = anchorX - (anchorX - offsetX) * ratio
    offsetY = anchorY - (anchorY - offsetY) * ratio
    scale = clamped
    if (scale <= MIN_SCALE) {
      offsetX = 0
      offsetY = 0
    }
  }

  function onWheel(event: WheelEvent) {
    event.preventDefault()
    const rect = imageContainer.getBoundingClientRect()
    const anchorX = event.clientX - (rect.left + rect.width / 2)
    const anchorY = event.clientY - (rect.top + rect.height / 2)
    const factor = event.deltaY < 0 ? 1.2 : 1 / 1.2
    applyZoom(scale * factor, anchorX, anchorY)
  }

  function zoomIn() {
    applyZoom(scale * 1.5, 0, 0)
  }

  function zoomOut() {
    applyZoom(scale / 1.5, 0, 0)
  }

  function onPointerDown(event: PointerEvent) {
    if (scale <= MIN_SCALE) return
    dragging = true
    dragStartX = event.clientX
    dragStartY = event.clientY
    dragOriginX = offsetX
    dragOriginY = offsetY
    imageElement?.setPointerCapture?.(event.pointerId)
  }

  function onPointerMove(event: PointerEvent) {
    if (!dragging) return
    offsetX = dragOriginX + (event.clientX - dragStartX)
    offsetY = dragOriginY + (event.clientY - dragStartY)
  }

  function onPointerUp() {
    dragging = false
  }

  function releaseMedia() {
    if (imageUrl) URL.revokeObjectURL(imageUrl)
    imageUrl = ''
    resetZoom()
  }

  onDestroy(() => {
    loadGeneration += 1
    releaseMedia()
  })
</script>

<Dialog.Root bind:open>
  <Dialog.Content
    class="grid h-[min(820px,calc(100vh-3rem))] max-w-[min(1040px,calc(100vw-3rem))] grid-rows-[auto_minmax(0,1fr)] gap-0 overflow-hidden p-0 sm:max-w-[min(1040px,calc(100vw-3rem))]"
  >
    <Dialog.Header class="border-b px-6 py-4 pr-14">
      <Dialog.Title class="truncate">{attachment?.file_name ?? tr('Attachment preview')}</Dialog.Title>
      <Dialog.Description class="mt-1">
        {#if attachment}
          {attachment.media_type} · {(attachment.byte_size / 1024).toFixed(1)} KiB
        {:else}
          {tr('Review attachments from the agent')}
        {/if}
      </Dialog.Description>
    </Dialog.Header>

    <div class="min-h-0 bg-muted/20 p-4">
      {#if loading}
        <div class="grid h-full place-items-center text-muted-foreground">
          <div class="flex items-center gap-2 text-xs">
            <LoaderCircle class="size-4 animate-spin" />
            {tr('Loading attachment…')}
          </div>
        </div>
      {:else if error}
        <div class="grid h-full place-items-center text-center">
          <div class="max-w-sm">
            <AlertCircle class="mx-auto size-6 text-destructive" />
            <strong class="mt-3 block text-sm">{tr('Unable to preview attachment')}</strong>
            <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">{error}</p>
          </div>
        </div>
      {:else if attachment?.media_type === 'text/markdown'}
        <MarkdownPreview {markdown} />
      {:else if imageUrl}
        <div
          bind:this={imageContainer}
          class="relative grid h-full min-h-0 touch-none place-items-center overflow-hidden rounded-lg border bg-[repeating-conic-gradient(hsl(var(--muted))_0_25%,transparent_0_50%)_50%/16px_16px]"
          onwheel={onWheel}
        >
          <img
            bind:this={imageElement}
            src={imageUrl}
            alt={attachment?.file_name ?? tr('Image attachment')}
            draggable="false"
            class={[
              'max-h-full max-w-full select-none object-contain shadow-sm',
              scale > 1 ? (dragging ? 'cursor-grabbing' : 'cursor-grab') : 'cursor-zoom-in',
            ]}
            style={`transform: translate(${offsetX}px, ${offsetY}px) scale(${scale}); ${dragging ? '' : 'transition: transform 120ms ease-out;'}`}
            onpointerdown={onPointerDown}
            onpointermove={onPointerMove}
            onpointerup={onPointerUp}
            onpointercancel={onPointerUp}
          />
          <div class="absolute bottom-3 right-3 flex items-center gap-1 rounded-lg border bg-background/90 p-0.5 shadow-sm">
            <Button
              variant="ghost"
              size="icon-sm"
              disabled={scale <= MIN_SCALE}
              aria-label={tr('Zoom out')}
              title={tr('Zoom out')}
              onclick={zoomOut}
            >
              <Minus />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              disabled={scale >= MAX_SCALE}
              aria-label={tr('Zoom in')}
              title={tr('Zoom in')}
              onclick={zoomIn}
            >
              <Plus />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              disabled={scale <= MIN_SCALE}
              aria-label={tr('Reset zoom')}
              title={tr('Reset zoom')}
              onclick={resetZoom}
            >
              <RotateCcw />
            </Button>
          </div>
        </div>
      {:else if unsupported}
        <div class="grid h-full place-items-center text-center">
          <div class="max-w-sm">
            <FileQuestion class="mx-auto size-6 text-muted-foreground" />
            <strong class="mt-3 block text-sm">{tr('This file type cannot be previewed.')}</strong>
            <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
              {attachment?.file_name}
            </p>
            {#if openMessage}
              <p class="m-0 mt-2 text-xs text-emerald-600">{openMessage}</p>
            {/if}
            {#if openError}
              <p class="m-0 mt-2 break-all text-xs leading-5 text-muted-foreground">{openError}</p>
            {/if}
            <Button class="mt-4" onclick={() => void openExternally()}>
              <ExternalLink class="size-4" />
              {tr('Open with the system default app')}
            </Button>
          </div>
        </div>
      {/if}
    </div>
  </Dialog.Content>
</Dialog.Root>

<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import {
    AlertCircle,
    ExternalLink,
    FileQuestion,
    FolderOpen,
    LoaderCircle,
    Minus,
    Plus,
    RotateCcw,
  } from '@lucide/svelte'
  import { onDestroy, tick } from 'svelte'

  import { Button } from '$lib/components/ui/button'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import * as Dialog from '$lib/components/ui/dialog'
  import { toast } from '$lib/components/ui/sonner'
  import type { AttachmentView, RequestAttachmentView } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import {
    clampImageZoom,
    computeImagePreviewZoom,
    imageDisplaySize,
  } from './imagePreviewZoom'
  import MarkdownPreview from './MarkdownPreview.svelte'

  export let open = false
  export let transport: ApplicationTransport
  export let requestId = ''
  export let readKind: 'request' | 'workspace' = 'request'
  export let attachment: (RequestAttachmentView & AttachmentView) | null = null

  let loading = false
  let error = ''
  let markdown = ''
  let imageUrl = ''
  let imageViewport: HTMLDivElement
  let imageViewportWidth = 0
  let imageViewportHeight = 0
  let imageNaturalWidth = 0
  let imageNaturalHeight = 0
  let zoom = 1
  let zoomInitialized = false
  let unsupported = false
  let openMessage = ''
  let openError = ''
  let revealBusy = false
  let loadedKey = ''
  let loadGeneration = 0

  $: requestedKey = open && attachment ? `${requestId}:${attachment.attachment_id}` : ''
  $: if (requestedKey && requestedKey !== loadedKey) void loadAttachment(requestedKey)
  $: if (!open && loadedKey) resetPreview()
  $: zoomModel = computeImagePreviewZoom({
    naturalWidth: imageNaturalWidth,
    naturalHeight: imageNaturalHeight,
    viewportWidth: imageViewportWidth,
    viewportHeight: imageViewportHeight,
  })
  $: imageSize = imageDisplaySize(imageNaturalWidth, imageNaturalHeight, zoom)
  $: if (
    imageUrl &&
    imageNaturalWidth > 0 &&
    imageNaturalHeight > 0 &&
    imageViewportWidth > 0 &&
    imageViewportHeight > 0 &&
    !zoomInitialized
  ) {
    zoom = zoomModel.initialZoom
    zoomInitialized = true
  }
  $: if (zoomInitialized && zoom < zoomModel.minZoom) zoom = zoomModel.minZoom
  $: if (zoomInitialized && zoom > zoomModel.maxZoom) zoom = zoomModel.maxZoom
  $: canZoomOut = zoom > zoomModel.minZoom + 0.001
  $: canZoomIn = zoom < zoomModel.maxZoom - 0.001
  $: canResetZoom = Math.abs(zoom - zoomModel.initialZoom) > 0.001

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
          ? await transport.call('readFeedbackAttachment', {
              request_id: requestId,
              attachment_id: current.attachment_id,
            })
          : await transport.call('readRequestAttachment', {
              request_id: requestId,
              attachment_id: current.attachment_id,
            })
      if (generation !== loadGeneration || !open) return
      const buffer =
        raw instanceof ArrayBuffer ? new Uint8Array(raw) : Uint8Array.from(raw)
      if (current.media_type === 'text/markdown') {
        markdown = new TextDecoder('utf-8', { fatal: true }).decode(buffer)
      } else if (current.media_type.startsWith('image/')) {
        const nextImageUrl = URL.createObjectURL(new Blob([buffer], { type: current.media_type }))
        let dimensions: { width: number; height: number }
        try {
          dimensions = await imageDimensions(nextImageUrl)
        } catch (cause) {
          URL.revokeObjectURL(nextImageUrl)
          throw cause
        }
        if (generation !== loadGeneration || !open) {
          URL.revokeObjectURL(nextImageUrl)
          return
        }
        imageUrl = nextImageUrl
        imageNaturalWidth = dimensions.width
        imageNaturalHeight = dimensions.height
        zoomInitialized = false
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

  async function imageDimensions(source: string): Promise<{ width: number; height: number }> {
    const probe = new Image()
    probe.src = source
    await probe.decode()
    return {
      width: probe.naturalWidth || probe.width || 1,
      height: probe.naturalHeight || probe.height || 1,
    }
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
    revealBusy = false
    imageNaturalWidth = 0
    imageNaturalHeight = 0
    zoomInitialized = false
    releaseMedia()
  }

  async function revealAttachmentInFolder() {
    const current = attachment
    if (!current || !requestId || revealBusy) return
    revealBusy = true
    try {
      const path = await invoke<string>('reveal_feedback_attachment', {
        input: {
          requestId,
          attachmentId: current.attachment_id,
          kind: readKind,
        },
      })
      toast.success(tr('Attachment shown in folder'), { description: path })
    } catch (cause) {
      toast.error(tr('Could not show the file in the folder'), {
        description: messageFrom(cause),
      })
    } finally {
      revealBusy = false
    }
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

  function resetZoom() {
    zoom = zoomModel.initialZoom
    zoomInitialized = true
    void tick().then(() => {
      if (!imageViewport) return
      imageViewport.scrollLeft = 0
      imageViewport.scrollTop = 0
    })
  }

  function applyZoom(nextZoom: number, anchorX?: number, anchorY?: number) {
    const clamped = clampImageZoom(nextZoom, zoomModel)
    if (Math.abs(clamped - zoom) <= 0.001) return
    const viewport = imageViewport
    const previousZoom = zoom
    const hasAnchor = viewport && anchorX !== undefined && anchorY !== undefined
    const contentX = hasAnchor ? viewport.scrollLeft + anchorX : 0
    const contentY = hasAnchor ? viewport.scrollTop + anchorY : 0
    const ratio = clamped / previousZoom
    zoom = clamped
    zoomInitialized = true
    if (hasAnchor) {
      void tick().then(() => {
        viewport.scrollLeft = contentX * ratio - anchorX
        viewport.scrollTop = contentY * ratio - anchorY
      })
    }
  }

  function onWheel(event: WheelEvent) {
    if (!event.ctrlKey && !event.metaKey) return
    event.preventDefault()
    const rect = imageViewport.getBoundingClientRect()
    const anchorX = event.clientX - rect.left
    const anchorY = event.clientY - rect.top
    const factor = event.deltaY < 0 ? 1.2 : 1 / 1.2
    applyZoom(zoom * factor, anchorX, anchorY)
  }

  function zoomIn() {
    applyZoom(zoom * 1.5, imageViewportWidth / 2, imageViewportHeight / 2)
  }

  function zoomOut() {
    applyZoom(zoom / 1.5, imageViewportWidth / 2, imageViewportHeight / 2)
  }

  function releaseMedia() {
    if (imageUrl) URL.revokeObjectURL(imageUrl)
    imageUrl = ''
    zoom = 1
    zoomInitialized = false
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
      <div class="flex min-w-0 items-start gap-3">
        <div class="min-w-0 flex-1">
          <Dialog.Title class="truncate">{attachment?.file_name ?? tr('Attachment preview')}</Dialog.Title>
          <Dialog.Description class="mt-1">
            {#if attachment}
              {attachment.media_type} · {(attachment.byte_size / 1024).toFixed(1)} KiB
            {:else}
              {tr('Review attachments from the agent')}
            {/if}
          </Dialog.Description>
        </div>
        {#if attachment}
          <Button
            class="shrink-0"
            variant="outline"
            size="sm"
            disabled={revealBusy}
            onclick={() => void revealAttachmentInFolder()}
          >
            {#if revealBusy}
              <LoaderCircle class="animate-spin" data-icon="inline-start" />
              {tr('Showing in folder…')}
            {:else}
              <FolderOpen data-icon="inline-start" />
              {tr('Show in folder')}
            {/if}
          </Button>
        {/if}
      </div>
    </Dialog.Header>

    <div class="h-full min-h-0 overflow-hidden bg-muted/20 p-4">
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
        <div class="relative h-full min-h-0 overflow-hidden rounded-lg border bg-[repeating-conic-gradient(hsl(var(--muted))_0_25%,transparent_0_50%)_50%/16px_16px]">
          <div
            bind:this={imageViewport}
            bind:clientWidth={imageViewportWidth}
            bind:clientHeight={imageViewportHeight}
            class="h-full min-h-0 overflow-auto overscroll-contain p-3"
            onwheel={onWheel}
          >
            {#if imageSize}
              <img
                src={imageUrl}
                alt={attachment?.file_name ?? tr('Image attachment')}
                draggable="false"
                class="mx-auto block max-w-none select-none object-contain shadow-sm"
                style={`width: ${imageSize.width}px; height: ${imageSize.height}px;`}
              />
            {/if}
          </div>
          <div class="absolute bottom-3 right-3 flex items-center gap-1 rounded-lg border bg-background/90 p-0.5 shadow-sm">
            <Button
              variant="ghost"
              size="icon-sm"
              disabled={!canZoomOut}
              aria-label={tr('Zoom out')}
              title={tr('Zoom out')}
              onclick={zoomOut}
            >
              <Minus />
            </Button>
            <span class="min-w-10 px-1 text-center font-mono text-[10px] text-muted-foreground">
              {Math.round(zoom * 100)}%
            </span>
            <Button
              variant="ghost"
              size="icon-sm"
              disabled={!canZoomIn}
              aria-label={tr('Zoom in')}
              title={tr('Zoom in')}
              onclick={zoomIn}
            >
              <Plus />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              disabled={!canResetZoom}
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

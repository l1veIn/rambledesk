<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { AlertCircle, Expand, LoaderCircle } from '@lucide/svelte'
  import { onDestroy, tick } from 'svelte'
  import Viewer from 'viewerjs'
  import 'viewerjs/dist/viewer.css'

  import { Button } from '$lib/components/ui/button'
  import * as Dialog from '$lib/components/ui/dialog'
  import type { RequestAttachmentView } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import MarkdownPreview from './MarkdownPreview.svelte'

  export let open = false
  export let requestId = ''
  export let attachment: RequestAttachmentView | null = null

  let loading = false
  let error = ''
  let markdown = ''
  let imageUrl = ''
  let imageElement: HTMLImageElement
  let viewer: Viewer | null = null
  let viewerReady = false
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
    try {
      const bytes = await invoke<ArrayBuffer>('read_request_attachment', {
        requestId,
        attachmentId: current.attachment_id,
      })
      if (generation !== loadGeneration || !open) return
      const buffer = new Uint8Array(bytes)
      if (current.media_type === 'text/markdown') {
        markdown = new TextDecoder('utf-8', { fatal: true }).decode(buffer)
      } else if (current.media_type.startsWith('image/')) {
        imageUrl = URL.createObjectURL(new Blob([buffer], { type: current.media_type }))
        await tick()
        if (generation !== loadGeneration || !imageElement) return
        await imageElement.decode()
        if (generation !== loadGeneration || !open) return
        viewer = new Viewer(imageElement, {
          backdrop: true,
          button: true,
          navbar: false,
          title: () => current.file_name,
          toolbar: {
            zoomIn: true,
            zoomOut: true,
            oneToOne: true,
            reset: true,
            prev: false,
            play: false,
            next: false,
            rotateLeft: true,
            rotateRight: true,
            flipHorizontal: true,
            flipVertical: true,
          },
        })
        viewerReady = true
      } else {
        throw new Error(tr('不支持预览这种附件格式。'))
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
    return tr('读取附件时发生未知错误。')
  }

  function resetPreview() {
    loadGeneration += 1
    loadedKey = ''
    loading = false
    error = ''
    markdown = ''
    releaseMedia()
  }

  function releaseMedia() {
    viewerReady = false
    viewer?.destroy()
    viewer = null
    if (imageUrl) URL.revokeObjectURL(imageUrl)
    imageUrl = ''
  }

  function openImageViewer() {
    if (!viewerReady) return
    viewer?.show(true)
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
      <Dialog.Title class="truncate">{attachment?.file_name ?? tr('附件预览')}</Dialog.Title>
      <Dialog.Description class="mt-1">
        {#if attachment}
          {attachment.media_type} · {(attachment.byte_size / 1024).toFixed(1)} KiB
        {:else}
          {tr('Agent 提供的评审附件')}
        {/if}
      </Dialog.Description>
    </Dialog.Header>

    <div class="min-h-0 bg-muted/20 p-4">
      {#if loading}
        <div class="grid h-full place-items-center text-muted-foreground">
          <div class="flex items-center gap-2 text-xs">
            <LoaderCircle class="size-4 animate-spin" />
            {tr('正在载入附件…')}
          </div>
        </div>
      {:else if error}
        <div class="grid h-full place-items-center text-center">
          <div class="max-w-sm">
            <AlertCircle class="mx-auto size-6 text-destructive" />
            <strong class="mt-3 block text-sm">{tr('无法预览附件')}</strong>
            <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">{error}</p>
          </div>
        </div>
      {:else if attachment?.media_type === 'text/markdown'}
        <MarkdownPreview {markdown} />
      {:else if imageUrl}
        <div class="relative grid h-full min-h-0 place-items-center overflow-hidden rounded-lg border bg-[repeating-conic-gradient(hsl(var(--muted))_0_25%,transparent_0_50%)_50%/16px_16px] p-4">
          <button
            type="button"
            class="contents"
            aria-label={tr('缩放查看')}
            disabled={!viewerReady}
            onclick={openImageViewer}
          >
            <img
              bind:this={imageElement}
              src={imageUrl}
              alt={attachment?.file_name ?? tr('图片附件')}
              class="max-h-full max-w-full cursor-zoom-in object-contain shadow-sm"
            />
          </button>
          <Button
            variant="secondary"
            size="sm"
            class="absolute bottom-3 right-3 shadow-sm"
            disabled={!viewerReady}
            onclick={openImageViewer}
          >
            <Expand />
            {tr('缩放查看')}
          </Button>
        </div>
      {/if}
    </div>
  </Dialog.Content>
</Dialog.Root>

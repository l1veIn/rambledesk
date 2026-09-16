<script lang="ts">
  import { Camera, ClipboardPaste, Paperclip } from '@lucide/svelte'

  import { Button } from '$lib/components/ui/button'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'

  export let attachmentBusy = false
  export let readOnly = false
  export let nativeCaptureAvailable = false
  export let onScreenCapture: () => void = () => {}
  export let onImportClipboard: () => void = () => {}
  export let onFileSelection: (event: Event) => void = () => {}

  let attachmentInput: HTMLInputElement

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }
</script>

<div class="flex shrink-0 items-center gap-1">
  {#if nativeCaptureAvailable}
    <Button
      variant="ghost"
      size="sm"
      class="h-8 gap-1.5 px-2 text-xs"
      disabled={attachmentBusy || readOnly}
      onclick={onScreenCapture}
      aria-label={tr('Capture')}
      title={tr('Capture')}
    >
      <Camera class="size-4" />
      <span class="hidden @min-[640px]:inline">{tr('Capture')}</span>
    </Button>
    <Button
      variant="ghost"
      size="sm"
      class="h-8 gap-1.5 px-2 text-xs"
      disabled={attachmentBusy || readOnly}
      onclick={onImportClipboard}
      aria-label={tr('Clipboard')}
      title={tr('The clipboard is read once only when you click import.')}
    >
      <ClipboardPaste class="size-4" />
      <span class="hidden @min-[640px]:inline">{tr('Clipboard')}</span>
    </Button>
  {/if}
  <Button
    variant="ghost"
    size="sm"
    class="h-8 gap-1.5 px-2 text-xs"
    disabled={attachmentBusy || readOnly}
    onclick={() => attachmentInput.click()}
    aria-label={tr('Choose files')}
    title={tr('Choose files')}
  >
    <Paperclip class="size-4" />
    <span class="hidden @min-[640px]:inline">{tr('Files')}</span>
  </Button>
</div>
<input
  bind:this={attachmentInput}
  hidden
  type="file"
  multiple
  disabled={attachmentBusy || readOnly}
  onchange={onFileSelection}
/>

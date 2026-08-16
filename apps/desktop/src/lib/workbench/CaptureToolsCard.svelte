<script lang="ts">
  import { Camera, ClipboardPaste, Paperclip } from '@lucide/svelte'

  import { Button } from '$lib/components/ui/button'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'

  export let attachmentCount = 0
  export let attachmentBusy = false
  export let rambleEngaged = false
  export let readOnly = false
  export let onScreenCapture: () => void = () => {}
  export let onImportClipboard: () => void = () => {}
  export let onFileSelection: (event: Event) => void = () => {}

  let attachmentInput: HTMLInputElement

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }
</script>

<section class="border-b p-4">
  <header class="mb-2 flex items-center gap-2">
    <Paperclip class="size-4 text-muted-foreground" />
    <strong class="text-xs font-medium">{tr('Add context')}</strong>
    <span class="ml-auto text-[10px] tabular-nums text-muted-foreground">{attachmentCount}</span>
  </header>
  <div class="grid grid-cols-3 gap-1.5">
    <Button
      variant="outline"
      class="h-14 flex-col gap-1 px-1 text-[10px]"
      disabled={!rambleEngaged || attachmentBusy || readOnly}
      onclick={onScreenCapture}
      title={tr('Capture')}
    >
      <Camera class="size-4" />
      {tr('Capture')}
    </Button>
    <Button
      variant="outline"
      class="h-14 flex-col gap-1 px-1 text-[10px]"
      disabled={!rambleEngaged || attachmentBusy || readOnly}
      onclick={onImportClipboard}
      title={tr('Clipboard')}
    >
      <ClipboardPaste class="size-4" />
      {tr('Clipboard')}
    </Button>
    <Button
      variant="outline"
      class="h-14 flex-col gap-1 px-1 text-[10px]"
      disabled={attachmentBusy || readOnly}
      onclick={() => attachmentInput.click()}
      title={tr('Choose files')}
    >
      <Paperclip class="size-4" />
      {tr('Files')}
    </Button>
  </div>
  <input
    bind:this={attachmentInput}
    class="sr-only"
    type="file"
    multiple
    onchange={onFileSelection}
  />
  <p class="m-0 mt-2 text-[9px] leading-4 text-muted-foreground">
    {tr('The clipboard is read once only when you click import.')}
  </p>
</section>

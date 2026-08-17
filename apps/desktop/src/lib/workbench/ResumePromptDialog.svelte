<script lang="ts">
  import { Check, Clipboard, ClipboardX } from '@lucide/svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import * as Dialog from '$lib/components/ui/dialog'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import type { ResumePrompt } from './types'

  export let prompt: ResumePrompt
  export let copyState: 'idle' | 'copied' | 'failed' = 'idle'
  export let onCopy: () => void = () => {}
  export let onDismiss: () => void = () => {}

  const displayedPrompt = { ...prompt }
  let dialogOpen = true
  let closeDelivered = false

  $: if (!dialogOpen && !closeDelivered) {
    closeDelivered = true
    onDismiss()
  }

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function requestDismiss() {
    dialogOpen = false
  }
</script>

<Dialog.Root bind:open={dialogOpen}>
  <Dialog.Content class="max-w-lg gap-5 sm:max-w-lg">
    <Dialog.Header>
      <div class="mb-1 flex items-center gap-2">
        <Badge variant="outline">Continuation</Badge>
        <Badge variant="secondary">{tr('Click Continue first')}</Badge>
      </div>
      <Dialog.Title>{displayedPrompt.title}</Dialog.Title>
      <Dialog.Description class="leading-5">{displayedPrompt.body}</Dialog.Description>
    </Dialog.Header>

    <dl class="grid grid-cols-[88px_minmax(0,1fr)] gap-x-3 gap-y-2 border-y py-3 text-xs">
      <dt class="text-muted-foreground">{tr('Hosts')}</dt>
      <dd class="m-0 font-medium">{displayedPrompt.host_label}</dd>
      <dt class="text-muted-foreground">request_id</dt>
      <dd class="m-0 truncate font-mono text-[10px]" title={displayedPrompt.request_id}>
        {displayedPrompt.request_id}
      </dd>
    </dl>

    <label class="grid gap-2 text-xs font-medium" for="resume-prompt-text">
      {tr('Fallback resume prompt (only if the host did not wait)')}
      <textarea
        id="resume-prompt-text"
        class="min-h-24 w-full resize-none rounded-md border bg-muted/45 p-3 font-mono text-[11px] font-normal leading-5 outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
        readonly
        value={displayedPrompt.resume_prompt}
      ></textarea>
    </label>

    <Dialog.Footer>
      <Button variant="outline" onclick={requestDismiss}>{tr('Close')}</Button>
      <Button onclick={onCopy}>
        {#if copyState === 'copied'}
          <Check data-icon="inline-start" />
          {tr('Copied')}
        {:else if copyState === 'failed'}
          <ClipboardX data-icon="inline-start" />
          {tr('Copy failed')}
        {:else}
          <Clipboard data-icon="inline-start" />
          {tr('Copy resume prompt')}
        {/if}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

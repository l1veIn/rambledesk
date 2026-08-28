<script lang="ts">
  import type { JSONContent } from '@tiptap/core'
  import { ChevronDown, MessageSquareText } from '@lucide/svelte'

  import * as Collapsible from '$lib/components/ui/collapsible'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import MarkdownPreview from './MarkdownPreview.svelte'

  export let document: JSONContent
  export let groupCount = 1
  export let previews: Record<string, string> = {}
  export let onOpenAttachment: (attachmentId: string) => void = () => {}

  let open = true

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }
</script>

<Collapsible.Root
  bind:open
  class="mt-3 overflow-hidden rounded-lg border border-border/80 bg-background/75 shadow-xs"
>
  <Collapsible.Trigger
    type="button"
    class="flex w-full items-center gap-2 px-3 py-2 text-left text-xs transition-colors hover:bg-accent/45 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring"
    aria-label={open ? tr('Collapse') : tr('Expand')}
  >
    <MessageSquareText class="size-3.5 shrink-0 text-muted-foreground" />
    <span class="font-medium">{tr('Action feedback')}</span>
    <span class="text-[11px] text-muted-foreground">
      · {tr('{count} segments', { count: groupCount })}
    </span>
    <ChevronDown
      class={[
        'ml-auto size-3.5 shrink-0 text-muted-foreground transition-transform',
        open ? 'rotate-180' : '',
      ]}
    />
  </Collapsible.Trigger>

  <Collapsible.Content>
    <div class="max-h-64 overflow-y-auto overscroll-contain border-t bg-muted/15">
      <MarkdownPreview
        {document}
        compact
        {previews}
        {onOpenAttachment}
      />
    </div>
  </Collapsible.Content>
</Collapsible.Root>

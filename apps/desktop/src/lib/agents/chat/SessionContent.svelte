<script lang="ts">
  import type { SessionContentBlock } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'
  import { chatText } from './chat-text'
  import { inlineMediaSource } from './activity-presentation'
  import DiffPreview from './DiffPreview.svelte'
  import MessageMarkdown from './MessageMarkdown.svelte'
  import UserMessageText from './UserMessageText.svelte'
  import ResourceLink from './ResourceLink.svelte'

  export let blocks: readonly SessionContentBlock[]
  export let userMessage = false
  export let truncated = false
</script>

<div class="min-w-0 space-y-3 break-words text-sm leading-7">
  {#each blocks as block}
    {#if block.type === 'text'}
      {#if userMessage}<UserMessageText text={block.text} />{:else}<MessageMarkdown text={block.text} />{/if}
    {:else if block.type === 'diff'}
      <DiffPreview path={block.path} oldText={block.old_text} newText={block.new_text} />
    {:else if block.type === 'image'}
      {@const source = inlineMediaSource(block)}
      <figure class="m-0 space-y-1 rounded-md border p-2">
        {#if source}<img src={source} alt={chatText($locale, 'Image output')} class="max-h-80 max-w-full rounded object-contain" loading="lazy" />
        {:else}<p class="m-0 text-xs text-muted-foreground">{chatText($locale, 'Media preview unavailable')} · {block.mime_type}</p>{/if}
        {#if block.uri}<figcaption class="text-xs"><ResourceLink uri={block.uri} /></figcaption>{/if}
      </figure>
    {:else if block.type === 'audio'}
      {@const source = inlineMediaSource(block)}
      {#if source}
        <!-- svelte-ignore a11y_media_has_caption (ACP supplies audio bytes without transcript tracks.) -->
        <audio controls src={source} aria-label={chatText($locale, 'Audio output')} class="max-w-full"></audio>
      {:else}<p class="m-0 text-xs text-muted-foreground">{chatText($locale, 'Media preview unavailable')} · {block.mime_type}</p>{/if}
    {:else if block.type === 'resource'}
      <section class="space-y-2 rounded-md border p-3 text-xs" aria-label={chatText($locale, 'Resource')}>
        {#if block.name}<p class="m-0 font-medium">{block.name}</p>{/if}
        <ResourceLink uri={block.uri} />
        {#if block.text}<pre class="m-0 max-h-80 overflow-auto whitespace-pre-wrap break-words font-mono text-[11px] leading-5">{block.text}</pre>{/if}
      </section>
    {:else if block.type === 'terminal'}
      <p class="m-0 rounded border bg-muted/25 px-3 py-2 text-xs text-muted-foreground">{chatText($locale, 'Terminal reference')}: <code>{block.terminal_id}</code></p>
    {:else if block.type === 'unsupported'}
      <p class="m-0 text-xs text-muted-foreground">{chatText($locale, 'Unsupported content')}: {block.label}</p>
    {/if}
  {/each}
  {#if truncated}<p class="m-0 text-[11px] text-muted-foreground">{chatText($locale, 'Content truncated by the agent host')}</p>{/if}
</div>

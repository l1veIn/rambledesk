<!-- Adapted from Codeg src/components/message/plain-text-with-badges.tsx and message-quote.ts at 3ebdfed.
     SPDX-License-Identifier: Apache-2.0; RambleDesk keeps plain prose and quote structure in Svelte. -->
<script lang="ts">
  import { parseQuoteBlocks, type QuoteBlock } from '../composer/message-quote'
  export let text = ''
  export let blocks: QuoteBlock[] | null = null
  $: rendered = blocks ?? parseQuoteBlocks(text)
</script>

<div class="space-y-2 text-sm leading-7">
  {#each rendered as block}
    {#if block.kind === 'text'}<p class="m-0 whitespace-pre-wrap break-words">{block.text}</p>
    {:else}<blockquote class="m-0 border-l-2 border-muted-foreground/40 pl-3 text-muted-foreground"><svelte:self blocks={block.children} /></blockquote>{/if}
  {/each}
</div>

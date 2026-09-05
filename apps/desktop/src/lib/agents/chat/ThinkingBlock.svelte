<!-- Adapted from Codeg src/components/ai-elements/reasoning.tsx at 3ebdfed. -->
<!-- SPDX-License-Identifier: Apache-2.0; Svelte disclosure follows stream phases without invented timing. -->
<script lang="ts">
  import { LoaderCircle } from '@lucide/svelte'
  import type { SessionContentBlock } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'
  import { chatText } from './chat-text'
  import SessionContent from './SessionContent.svelte'
  export let blocks: readonly SessionContentBlock[]
  export let streaming = false
  export let truncated = false
  let open = streaming
  let previousStreaming = streaming
  $: if (streaming !== previousStreaming) {
    open = streaming
    previousStreaming = streaming
  }
</script>

<details bind:open class="text-muted-foreground">
  <summary class="flex w-fit cursor-pointer list-none items-center gap-2 text-xs">
    {#if streaming}<LoaderCircle class="size-3 animate-spin" />{/if}
    <span>{chatText($locale, streaming ? 'Thinking' : 'Reasoning')}</span><span aria-hidden="true">{open ? '⌄' : '›'}</span>
  </summary>
  {#if open}<div class="mt-3 border-l-2 border-muted pl-3"><SessionContent {blocks} {truncated} /></div>{/if}
</details>

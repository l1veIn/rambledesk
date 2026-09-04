<script lang="ts">
  import { onMount } from 'svelte'
  import MarkdownPreview from '$lib/workbench/MarkdownPreview.svelte'
  import { activityMarkdownDocument } from './activity-markdown'
  export let text = ''
  let mounted = false
  onMount(() => { mounted = true })
  $: document = activityMarkdownDocument(text)
</script>

<div class="agent-message-markdown min-w-0 break-words">
  {#if mounted}<MarkdownPreview {document} compact />{:else}<p class="m-0 whitespace-pre-wrap">{text}</p>{/if}
</div>

<style>
  .agent-message-markdown :global(.action-feedback-markdown) { padding: 0; }
  .agent-message-markdown :global(.attachment-markdown-prose) { font-size: 0.875rem; line-height: 1.8; }
</style>

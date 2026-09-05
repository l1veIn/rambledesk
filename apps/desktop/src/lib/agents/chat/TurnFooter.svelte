<script lang="ts">
  import { onDestroy } from 'svelte'
  import { Check, Copy } from '@lucide/svelte'
  import { locale } from '$lib/preferences'
  import { chatText } from './chat-text'
  export let copyText = ''
  export let completedAt: string | null = null
  let copied = false
  let failed = false
  let timer: ReturnType<typeof setTimeout> | undefined
  $: date = completedAt && Number.isFinite(Date.parse(completedAt)) ? new Date(completedAt) : null
  $: shortTime = date ? new Intl.DateTimeFormat($locale, { hour: '2-digit', minute: '2-digit' }).format(date) : ''
  $: fullTime = date ? new Intl.DateTimeFormat($locale, { dateStyle: 'medium', timeStyle: 'medium' }).format(date) : ''
  async function copy() {
    try {
      await navigator.clipboard.writeText(copyText)
      failed = false
      copied = true
      clearTimeout(timer)
      timer = setTimeout(() => { copied = false }, 2000)
    } catch { failed = true }
  }
  onDestroy(() => clearTimeout(timer))
</script>

{#if copyText.trim() || date}
  <footer class="mt-2 flex items-center gap-2 text-[11px] text-muted-foreground" data-turn-footer>
    {#if copyText.trim()}<button type="button" class="rounded p-1.5 hover:bg-muted hover:text-foreground" aria-label={chatText($locale, copied ? 'Copied' : 'Copy reply')} title={chatText($locale, copied ? 'Copied' : 'Copy reply')} onclick={copy}>{#if copied}<Check class="size-3.5" />{:else}<Copy class="size-3.5" />{/if}</button>{/if}
    {#if date}<time datetime={completedAt ?? undefined} title={`${chatText($locale, 'Completed at')}: ${fullTime}`} class="tabular-nums">{shortTime}</time>{/if}
    {#if failed}<span role="alert">{chatText($locale, 'Could not copy the reply.')}</span>{/if}
  </footer>
{/if}

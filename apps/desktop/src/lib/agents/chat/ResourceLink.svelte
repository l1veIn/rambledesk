<script lang="ts">
  import { useWorkbenchCapabilities } from '$lib/capabilities/capabilityContext'
  import { isSafeHttpUrl } from '$lib/linkify'
  import { locale } from '$lib/preferences'
  import { chatText } from './chat-text'
  export let uri: string
  export let label = ''
  const capabilities = useWorkbenchCapabilities()
  let failed = false
  async function open(event: MouseEvent) {
    event.preventDefault()
    failed = false
    try { await capabilities.externalLinks.implementation.open(uri) } catch { failed = true }
  }
</script>

{#if isSafeHttpUrl(uri)}
  <a href={uri} target="_blank" rel="noopener noreferrer" onclick={open} class="break-all text-primary underline underline-offset-4">{label || uri}</a>
{:else}<span class="break-all font-mono">{label || uri}</span>{/if}
{#if failed}<span role="alert" class="ml-2 text-destructive">{chatText($locale, 'Could not open the link')}</span>{/if}

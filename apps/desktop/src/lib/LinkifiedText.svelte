<script lang="ts">
  import { splitTextWithUrls } from './linkify'
  import { openExternalUrl } from './openExternalUrl'

  export let text = ''

  $: segments = splitTextWithUrls(text)

  function handleClick(event: MouseEvent, href: string) {
    event.preventDefault()
    void openExternalUrl(href).catch((cause) => {
      console.warn('Could not open external URL', cause)
    })
  }
</script>

{#each segments as segment, index (`${index}:${segment.type}:${segment.value}`)}
  {#if segment.type === 'url'}
    <a
      href={segment.value}
      class="break-all text-primary underline underline-offset-2"
      rel="noreferrer"
      onclick={(event) => handleClick(event, segment.value)}
    >{segment.value}</a>
  {:else}{segment.value}{/if}
{/each}

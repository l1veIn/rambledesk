<script lang="ts">
  import { Bot } from '@lucide/svelte'

  export let agentId: string
  export let label: string
  export let iconSvg = ''
  export let size: 'sm' | 'md' = 'md'
</script>

<span
  class={[
    'grid shrink-0 place-items-center rounded-md border font-semibold shadow-sm',
    size === 'sm' ? 'size-6 text-[9px]' : 'size-8 text-[10px]',
    agentId === 'claude_code'
        ? 'border-orange-500/25 bg-orange-500/10 text-orange-700 dark:text-orange-300'
        : 'border-primary/20 bg-primary/10 text-primary [&>svg]:size-4',
  ]}
  title={label}
  aria-label={label}
>
  {#if iconSvg}
    {@html iconSvg}
  {:else}
    <Bot class={size === 'sm' ? 'size-3.5' : 'size-4'} />
  {/if}
</span>

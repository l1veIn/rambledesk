<script lang="ts">
  import type { SessionContextUsage as Usage } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'
  import { contextUsageDisplay } from './contextUsage'

  export let usage: Usage | null | undefined = undefined
  $: display = contextUsageDisplay(usage)
  $: label = $locale === 'zh-CN' ? '上下文' : 'Context'
  $: title = `${$locale === 'zh-CN' ? '智能体上报的上下文 token 用量' : 'Context tokens reported by the agent'}${display ? `: ${display.used} / ${display.size}` : ''}`
</script>

{#if display}
  <span class="inline-flex shrink-0 items-center gap-1.5 whitespace-nowrap text-[10px] tabular-nums text-muted-foreground" {title} data-context-usage>
    <span>{label} {display.percent}%</span>
  </span>
{/if}

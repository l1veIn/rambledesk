<script lang="ts">
  import { CodeXml, Gauge, Lightbulb } from '@lucide/svelte'
  import { locale } from '$lib/preferences'
  import { getSessionStarters } from './sessionStarters'

  export let disabled = false
  export let onSelect: (prompt: string) => void

  const icons = { brainstorm: Lightbulb, optimize: Gauge, develop: CodeXml }
  $: starters = getSessionStarters($locale)
  $: label = $locale === 'zh-CN' ? '从一个小目标开始' : 'Start with something simple'
  $: hint = $locale === 'zh-CN' ? '点击填入提示词，修改后再发送。' : 'Choose a prompt, make it yours, then send.'
</script>

<section aria-label={label} class="space-y-3" data-session-starters>
  <div class="grid gap-2.5 sm:grid-cols-3">
    {#each starters as starter (starter.id)}
      <button type="button" {disabled} aria-label={starter.title}
        class="group flex min-w-0 items-start gap-3 rounded-xl border border-border/70 bg-card/40 p-4 text-left transition-colors hover:border-primary/30 hover:bg-muted/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50"
        onclick={() => { if (!disabled) onSelect(starter.prompt) }}>
        <svelte:component this={icons[starter.id]} class="mt-0.5 size-4 shrink-0 text-muted-foreground transition-colors group-hover:text-primary" aria-hidden="true" />
        <span class="min-w-0 space-y-1">
          <span class="block text-sm font-medium">{starter.title}</span>
          <span class="block text-xs leading-5 text-muted-foreground">{starter.description}</span>
        </span>
      </button>
    {/each}
  </div>
  <p class="m-0 text-center text-[11px] text-muted-foreground">{hint}</p>
</section>

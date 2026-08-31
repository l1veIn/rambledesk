<script lang="ts">
  import { Check, CircleHelp, SkipForward } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import type { QuestionAttentionItem } from './types'

  export let item: QuestionAttentionItem
  export let busy = false
  export let answerable = true
  export let onAnswer: (choiceIds: string[], skipped: boolean) => void = () => {}

  let selected = new Set<string>()
  let loadedItemId = ''
  $: if (item.id !== loadedItemId) {
    loadedItemId = item.id
    selected = new Set()
  }

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function toggle(choiceId: string) {
    if (!item.multiple) {
      selected = new Set([choiceId])
      return
    }
    const next = new Set(selected)
    if (next.has(choiceId)) next.delete(choiceId)
    else next.add(choiceId)
    selected = next
  }
</script>

<section class="flex h-full min-h-0 flex-col bg-background">
  <header class="flex min-h-16 shrink-0 items-center gap-3 border-b px-5 py-3">
    <span class="grid size-9 shrink-0 place-items-center rounded-md bg-info/15 text-info"><CircleHelp class="size-4" /></span>
    <div class="min-w-0 flex-1">
      <p class="m-0 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">{tr('Ask Question')}</p>
      <h1 class="m-0 truncate text-sm font-semibold">{item.title}</h1>
    </div>
  </header>

  <div class="min-h-0 flex-1 overflow-y-auto p-6">
    <form class="mx-auto max-w-2xl" onsubmit={(event) => { event.preventDefault(); if (answerable) onAnswer([...selected], false) }}>
      <p class="m-0 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">{tr('The Agent paused to ask you')}</p>
      <h2 class="mt-2 text-base font-semibold leading-6">{item.prompt}</h2>
      <p class="text-[11px] text-muted-foreground">{item.multiple ? tr('Choose one or more answers.') : tr('Choose one answer.')}</p>

      <div class="mt-5 space-y-2">
        {#each item.choices as choice}
          <label class={['flex cursor-pointer items-start gap-3 rounded-lg border p-3.5 transition-colors', selected.has(choice.id) ? 'border-primary bg-primary/5' : 'hover:bg-muted/50']}>
            <input
              type={item.multiple ? 'checkbox' : 'radio'}
              name={`question-${item.id}`}
              checked={selected.has(choice.id)}
              class="mt-0.5 accent-primary"
              onchange={() => toggle(choice.id)}
              disabled={busy || item.status !== 'waiting'}
            />
            <span class="min-w-0">
              <strong class="block text-xs font-medium">{choice.label}</strong>
              {#if choice.description}<span class="mt-1 block text-[11px] leading-4 text-muted-foreground">{choice.description}</span>{/if}
            </span>
          </label>
        {/each}
      </div>

      {#if item.status === 'waiting'}
        {#if !answerable}
          <p class="mt-6 rounded-md border bg-muted/25 px-4 py-3 text-xs text-muted-foreground">
            {tr('Answer the earlier request first.')}
          </p>
        {/if}
        <div class="mt-6 flex justify-end gap-2">
          {#if item.allowSkip}<Button type="button" variant="ghost" disabled={busy || !answerable} onclick={() => { if (answerable) onAnswer([], true) }}><SkipForward data-icon="inline-start" />{tr('Skip')}</Button>{/if}
          <Button type="submit" disabled={busy || !answerable || selected.size === 0}><Check data-icon="inline-start" />{tr('Send answer')}</Button>
        </div>
      {:else}
        <div class="mt-6 rounded-md border bg-muted/25 px-4 py-3 text-xs text-muted-foreground">{tr('This question has been answered.')}</div>
      {/if}
    </form>
  </div>
</section>

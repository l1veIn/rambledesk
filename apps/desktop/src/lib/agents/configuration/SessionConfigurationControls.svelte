<!-- Adapted from Codeg src/components/chat/session-config-selector.tsx at 3ebdfed. -->
<!-- SPDX-License-Identifier: Apache-2.0; Svelte/native controls, generated boolean values and Agent-confirmed selection. -->
<script lang="ts">
  import { LoaderCircle, ToggleLeft, ToggleRight } from '@lucide/svelte'
  import type { SessionConfigChange, SessionConfiguration } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'
  import { changeForControl, choiceGroups, configurationControls, type ConfigurationControl } from './configurationControls'

  export let configuration: SessionConfiguration
  export let disabled = false
  export let onChange: (change: SessionConfigChange) => Promise<void> | void
  let pending = false
  let failed = false
  $: controls = configurationControls(configuration)
  function tr(text: string) {
    const zh: Record<string, string> = { Mode: '模式', Model: '模型', On: '开启', Off: '关闭', 'Updating session options…': '正在更新会话选项…', 'Could not change this option.': '无法更改此选项。' }
    return $locale === 'zh-CN' ? zh[text] ?? text : text
  }

  async function change(control: ConfigurationControl, next: string | boolean) {
    const request = changeForControl(control, next)
    if (disabled || pending || !request) return
    pending = true
    failed = false
    try { await onChange(request) }
    catch { failed = true }
    finally { pending = false }
  }

  function select(event: Event, control: Extract<ConfigurationControl, { type: 'select' }>) {
    const element = event.currentTarget as HTMLSelectElement
    const next = element.value
    // An attempted change is not confirmation. Failed/refused requests keep the
    // last Agent value even when Svelte would otherwise leave the DOM selection.
    element.value = control.value
    void change(control, next)
  }
</script>

{#if controls.length}
  <div class="flex min-w-0 max-w-full flex-wrap items-center gap-1" data-session-config-controls>
    {#each controls as control (control.id)}
      {#if control.type === 'boolean'}
        <button type="button" class="flex min-w-0 items-center gap-1 rounded-md px-1.5 py-1 text-[11px] hover:bg-muted disabled:opacity-40" disabled={disabled || pending}
          aria-pressed={control.value} aria-label={`${tr(control.name)}: ${tr(control.value ? 'On' : 'Off')}`} title={control.description ?? control.name}
          onclick={() => void change(control, !control.value)}>
          {#if control.value}<ToggleRight class="size-3.5 shrink-0 text-primary" />{:else}<ToggleLeft class="size-3.5 shrink-0 text-muted-foreground" />{/if}
          <span class="max-w-32 truncate">{tr(control.name)}</span>
        </button>
      {:else}
        <label class="flex min-w-0 items-center gap-1 rounded-md px-1 py-1 text-[10px] text-muted-foreground" title={control.description ?? control.name}>
          <span class="sr-only">{tr(control.name)}</span>
          <select value={control.value} disabled={disabled || pending || control.choices.length === 0} aria-label={tr(control.name)}
            class="h-7 max-w-40 truncate rounded-md border-0 bg-transparent px-1 text-[11px] text-foreground outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-40"
            onchange={(event) => select(event, control)}>
            {#if !control.choices.some((choice) => choice.value === control.value)}<option value={control.value} disabled>{control.value || tr(control.name)}</option>{/if}
            {#each choiceGroups(control.choices) as group}
              {#if group.name}<optgroup label={group.name}>{#each group.choices as choice}<option value={choice.value} title={choice.description ?? choice.name}>{choice.name}</option>{/each}</optgroup>
              {:else}{#each group.choices as choice}<option value={choice.value} title={choice.description ?? choice.name}>{choice.name}</option>{/each}{/if}
            {/each}
          </select>
        </label>
      {/if}
    {/each}
    {#if pending}<LoaderCircle class="size-3 animate-spin text-muted-foreground" /><span role="status" class="sr-only">{tr('Updating session options…')}</span>{/if}
    {#if failed}<span role="alert" class="text-[10px] text-destructive">{tr('Could not change this option.')}</span>{/if}
  </div>
{/if}

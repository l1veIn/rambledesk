<!-- Adapted from Codeg src/components/chat/session-config-selector.tsx at 3ebdfed. -->
<!-- SPDX-License-Identifier: Apache-2.0; Svelte/native controls, generated boolean values and Agent-confirmed selection. -->
<script lang="ts">
  import { Popover } from 'bits-ui'
  import { Check, ChevronDown, LoaderCircle, ToggleLeft, ToggleRight } from '@lucide/svelte'
  import type { SessionConfigChange, SessionConfiguration } from '$lib/generated/feedback'
  import { PHONE_QUERY, mediaQuery } from '$lib/mediaQuery'
  import { locale } from '$lib/preferences'
  import {
    changeForControl,
    choiceGroups,
    configurationControls,
    controlDisplayValue,
    primaryConfigurationControl,
    type ConfigurationControl,
  } from './configurationControls'

  export let configuration: SessionConfiguration
  export let disabled = false
  export let onChange: (change: SessionConfigChange) => Promise<void> | void

  let pending = false
  let failed = false
  let pickerOpen = false
  const phone = mediaQuery(PHONE_QUERY)

  $: controls = configurationControls(configuration)
  $: primary = primaryConfigurationControl(controls)

  function tr(text: string) {
    const zh: Record<string, string> = { Mode: '模式', Model: '模型', 'Reasoning effort': '思考强度', On: '开启', Off: '关闭', 'Session options': '会话选项', 'Updating session options…': '正在更新会话选项…', 'Could not change this option.': '无法更改此选项。' }
    return $locale === 'zh-CN' ? zh[text] ?? text : text
  }

  function primaryLabel(): string {
    if (!primary) return tr('Session options')
    if (primary.type === 'boolean') {
      return `${tr(primary.name)} · ${tr(primary.value ? 'On' : 'Off')}`
    }
    return controlDisplayValue(primary)
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
  {#if $phone}
    <!-- Phones cannot afford a wrapping row of selectors: one entry opens every option. -->
    <Popover.Root bind:open={pickerOpen}>
      <Popover.Trigger>
        {#snippet child({ props })}
          <button
            {...props}
            type="button"
            class="flex h-8 min-w-0 max-w-52 shrink items-center gap-1 rounded-md px-1.5 text-[11px] text-foreground outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-40"
            data-session-config-compact
            disabled={disabled || pending}
            aria-label={`${tr('Session options')}: ${primaryLabel()}`}
            title={primary?.description ?? primaryLabel()}
          >
            <span class="min-w-0 truncate">{primaryLabel()}</span>
            <ChevronDown class="size-3 shrink-0 text-muted-foreground" aria-hidden="true" />
            {#if pending}<LoaderCircle class="size-3 shrink-0 animate-spin text-muted-foreground" />{/if}
          </button>
        {/snippet}
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content
          side="top"
          align="start"
          sideOffset={8}
          aria-label={tr('Session options')}
          data-session-config-compact-content
          class="z-[130] w-72 max-w-[calc(100vw-2rem)] rounded-xl border bg-popover p-1.5 text-popover-foreground shadow-lg outline-none"
        >
          <div class="max-h-[min(24rem,60vh)] overflow-y-auto">
            {#each controls as control (control.id)}
              <section class="border-b py-1 last:border-b-0">
                <p class="m-0 px-2 py-1 text-[11px] font-medium text-muted-foreground">{tr(control.name)}</p>
                {#if control.type === 'boolean'}
                  <button
                    type="button"
                    role="switch"
                    aria-checked={control.value}
                    class="flex w-full items-center gap-2 rounded-lg px-2 py-2 text-left text-xs hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
                    disabled={disabled || pending}
                    onclick={() => void change(control, !control.value)}
                  >
                    {#if control.value}<ToggleRight class="size-3.5 shrink-0 text-primary" />{:else}<ToggleLeft class="size-3.5 shrink-0 text-muted-foreground" />{/if}
                    <span class="min-w-0 flex-1 truncate">{tr(control.value ? 'On' : 'Off')}</span>
                    {#if control.value}<Check class="size-3.5 shrink-0" aria-hidden="true" />{/if}
                  </button>
                {:else}
                  <div role="radiogroup" aria-label={tr(control.name)}>
                    {#each choiceGroups(control.choices) as group}
                      {#if group.name}<p class="m-0 px-2 pb-1 pt-1.5 text-[10px] text-muted-foreground">{group.name}</p>{/if}
                      {#each group.choices as choice}
                        <button
                          type="button"
                          role="radio"
                          aria-checked={choice.value === control.value}
                          class="flex w-full items-center gap-2 rounded-lg px-2 py-2 text-left text-xs hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
                          disabled={disabled || pending}
                          title={choice.description ?? choice.name}
                          onclick={() => void change(control, choice.value)}
                        >
                          <span class="min-w-0 flex-1">
                            <span class="block truncate">{choice.name}</span>
                            {#if choice.description}<span class="mt-0.5 block text-[10px] text-muted-foreground">{choice.description}</span>{/if}
                          </span>
                          {#if choice.value === control.value}<Check class="size-3.5 shrink-0" aria-hidden="true" />{/if}
                        </button>
                      {/each}
                    {/each}
                  </div>
                {/if}
              </section>
            {/each}
          </div>
          {#if failed}<p class="m-0 px-2 py-1.5 text-[10px] text-destructive" role="alert">{tr('Could not change this option.')}</p>{/if}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  {:else}
    <div class="flex max-w-full flex-wrap items-center gap-1" data-session-config-controls>
      {#each controls as control (control.id)}
        {#if control.type === 'boolean'}
          <button type="button" class="flex shrink-0 items-center gap-1 rounded-md px-1.5 py-1 text-[11px] hover:bg-muted disabled:opacity-40" disabled={disabled || pending}
            aria-pressed={control.value} aria-label={`${tr(control.name)}: ${tr(control.value ? 'On' : 'Off')}`} title={control.description ?? control.name}
            onclick={() => void change(control, !control.value)}>
            {#if control.value}<ToggleRight class="size-3.5 shrink-0 text-primary" />{:else}<ToggleLeft class="size-3.5 shrink-0 text-muted-foreground" />{/if}
            <span class="min-w-0 max-w-32 truncate">{tr(control.name)}</span>
          </button>
        {:else}
          <label class="flex shrink-0 items-center gap-1 rounded-md px-1 py-1 text-[10px] text-muted-foreground" title={control.description ?? control.name}>
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
      {#if pending}<LoaderCircle class="size-3 shrink-0 animate-spin text-muted-foreground" /><span role="status" class="sr-only">{tr('Updating session options…')}</span>{/if}
      {#if failed}<span role="alert" class="shrink-0 text-[10px] text-destructive">{tr('Could not change this option.')}</span>{/if}
    </div>
  {/if}
{/if}

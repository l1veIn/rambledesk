<script lang="ts">
  import { AlertTriangle } from '@lucide/svelte'

  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'

  import type { LaunchConfigOption, LaunchConfigValue } from './types'

  export let option: LaunchConfigOption
  export let value: LaunchConfigValue | undefined
  export let disabled = false
  export let onChange: (value: LaunchConfigValue) => void = () => {}

  $: groupedChoices = option.kind === 'select'
    ? option.options.reduce<Map<string, typeof option.options>>((groups, choice) => {
        const group = choice.group ?? ''
        groups.set(group, [...(groups.get(group) ?? []), choice])
        return groups
      }, new Map())
    : new Map<string, never[]>()
  $: explicitGroups = option.kind === 'select' ? (option.groups ?? []) : []
  $: explicitlyGroupedValues = new Set(
    explicitGroups.flatMap((group) => group.options.map((choice) => choice.value)),
  )
  $: ungroupedChoices = option.kind === 'select'
    ? option.options.filter(
        (choice) => !explicitlyGroupedValues.has(choice.value) && !(choice.group ?? ''),
      )
    : []

  function tr(source: string) {
    return t($locale, source)
  }
</script>

{#if option.kind === 'select'}
  <label class="grid min-w-0 gap-1.5 text-[11px] font-medium">
    <span class="flex items-center justify-between gap-2">
      <span class="truncate">{option.name}</span>
      {#if option.source === 'profile'}
        <span class="shrink-0 text-[9px] font-normal text-muted-foreground">{tr('Launch setting')}</span>
      {/if}
    </span>
    <select
      value={typeof value === 'string' ? value : option.currentValue}
      {disabled}
      class="h-9 min-w-0 rounded-md border bg-background px-3 text-xs"
      aria-describedby={option.description ? `launch-config-${option.id}-description` : undefined}
      onchange={(event) => onChange(event.currentTarget.value)}
    >
      {#if explicitGroups.length > 0}
        {#each ungroupedChoices as choice}<option value={choice.value}>{choice.name}</option>{/each}
        {#each explicitGroups as group (group.id)}
          <optgroup label={group.name}>
            {#each group.options as choice}<option value={choice.value}>{choice.name}</option>{/each}
          </optgroup>
        {/each}
      {:else}
        {#each [...groupedChoices] as [group, choices]}
          {#if group}
            <optgroup label={group}>
              {#each choices as choice}<option value={choice.value}>{choice.name}</option>{/each}
            </optgroup>
          {:else}
            {#each choices as choice}<option value={choice.value}>{choice.name}</option>{/each}
          {/if}
        {/each}
      {/if}
    </select>
    {#if option.description}
      <span id={`launch-config-${option.id}-description`} class="text-[9px] font-normal leading-4 text-muted-foreground">
        {option.description}
      </span>
    {/if}
  </label>
{:else if option.kind === 'boolean'}
  <div class="flex min-h-14 items-center justify-between gap-4 rounded-md border bg-background px-3 py-2">
    <div class="min-w-0">
      <strong class="block truncate text-[11px] font-medium">{option.name}</strong>
      {#if option.description}
        <span class="mt-0.5 block text-[9px] leading-4 text-muted-foreground">{option.description}</span>
      {/if}
    </div>
    <button
      type="button"
      role="switch"
      aria-label={option.name}
      aria-checked={typeof value === 'boolean' ? value : option.currentValue}
      {disabled}
      class={[
        'relative h-[22px] w-10 shrink-0 rounded-full transition-colors disabled:opacity-50',
        (typeof value === 'boolean' ? value : option.currentValue) ? 'bg-primary' : 'bg-input',
      ]}
      onclick={() => onChange(!(typeof value === 'boolean' ? value : option.currentValue))}
    >
      <span
        class={[
          'absolute left-0.5 top-0.5 size-4 rounded-full bg-background shadow transition-transform',
          (typeof value === 'boolean' ? value : option.currentValue) ? 'translate-x-5' : '',
        ]}
      ></span>
    </button>
  </div>
{:else}
  <div class="flex min-h-14 items-start gap-2 rounded-md border border-warning/30 bg-warning/5 px-3 py-2 text-warning-foreground dark:text-warning">
    <AlertTriangle class="mt-0.5 size-3.5 shrink-0" />
    <div class="min-w-0">
      <strong class="block text-[11px]">{option.name}</strong>
      <span class="mt-0.5 block text-[9px] leading-4">
        {tr('This Agent option is not editable in this RambleDesk version. Its current Agent value will be kept.')}
      </span>
    </div>
  </div>
{/if}

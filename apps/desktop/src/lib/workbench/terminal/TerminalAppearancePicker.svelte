<script lang="ts">
  import { Check, Palette, X } from '@lucide/svelte'
  import { Popover } from 'bits-ui'
  import { Button } from '$lib/components/ui/button'
  import { locale } from '$lib/preferences'
  import { terminalText } from './terminalI18n'
  import { TERMINAL_APPEARANCES, setTerminalAppearance, terminalAppearance, type TerminalAppearanceId } from './terminalAppearance'

  let open = false
  let saved = true
  const tr = (source: string) => terminalText($locale, source)
  function choose(id: TerminalAppearanceId) { saved = setTerminalAppearance(id) }
</script>

<Popover.Root bind:open>
  <Popover.Trigger>
    {#snippet child({ props })}
      <Button {...props} size="sm" variant="outline" aria-label={tr('Terminal style')} title={tr('Terminal style')} data-terminal-appearance-trigger>
        <Palette class="size-4" aria-hidden="true" />{tr('Style')}
      </Button>
    {/snippet}
  </Popover.Trigger>
  <Popover.Portal>
    <Popover.Content side="bottom" align="end" sideOffset={8} aria-label={tr('Terminal style')} data-terminal-appearance-picker
      class="z-[130] w-80 max-w-[calc(100vw-1rem)] max-h-[var(--bits-popover-content-available-height)] overflow-y-auto rounded-xl border bg-popover p-4 text-popover-foreground shadow-lg outline-none">
      <div class="mb-3 flex items-center justify-between gap-2">
        <div><strong class="text-sm font-semibold">{tr('Terminal style')}</strong><p class="m-0 mt-1 text-[11px] text-muted-foreground">{tr('Choose a look for your terminal.')}</p></div>
        <Popover.Close>
          {#snippet child({ props })}<Button {...props} size="icon-xs" variant="ghost" aria-label={tr('Close')}><X aria-hidden="true" /></Button>{/snippet}
        </Popover.Close>
      </div>
      <div class="grid grid-cols-2 gap-2" role="group" aria-label={tr('Terminal color presets')}>
        {#each TERMINAL_APPEARANCES as appearance (appearance.id)}
          <button type="button" aria-label={tr(appearance.label)} aria-pressed={$terminalAppearance.id === appearance.id} data-terminal-appearance={appearance.id}
            class={['min-w-0 overflow-hidden rounded-lg border p-1.5 text-left transition-colors hover:border-primary/60 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring',
              $terminalAppearance.id === appearance.id ? 'border-primary bg-primary/5 ring-1 ring-primary/25' : 'border-border']}
            onclick={() => choose(appearance.id)}>
            <span class="block rounded-md p-2 font-mono text-[10px] leading-4" aria-hidden="true" style:background-color={appearance.theme.background} style:color={appearance.theme.foreground}>
              <span class="mb-2 flex gap-1" style:color={appearance.muted}><span class="size-1 rounded-full bg-current opacity-60"></span><span class="size-1 rounded-full bg-current opacity-40"></span><span class="size-1 rounded-full bg-current opacity-25"></span></span>
              <span class="block"><span style:color={appearance.theme.blue}>~/project</span> $</span>
              <span class="block" style:color={appearance.theme.green}>✓ {tr('Ready to try')}</span>
            </span>
            <span class="mt-1.5 flex items-center justify-between gap-1 px-1 text-xs font-medium">
              {tr(appearance.label)}<span class="grid size-4 place-items-center rounded-full" class:bg-primary={$terminalAppearance.id === appearance.id} class:text-primary-foreground={$terminalAppearance.id === appearance.id}>
                {#if $terminalAppearance.id === appearance.id}<Check class="size-3" aria-hidden="true" />{/if}
              </span>
            </span>
          </button>
        {/each}
      </div>
      <p class="m-0 mt-3 text-[11px] leading-4 text-muted-foreground" role="status">{tr(saved ? 'Colors apply immediately.' : 'Style applied. Could not save your preference.')}</p>
    </Popover.Content>
  </Popover.Portal>
</Popover.Root>

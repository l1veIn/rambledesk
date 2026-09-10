<script lang="ts">
  import { Image, LoaderCircle, Palette, RotateCcw, Scaling, Type } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import * as Select from '$lib/components/ui/select'
  import { locale, setThemePreference, themePreference, type ThemePreference } from '$lib/preferences'
  import {
    appearancePreferences, updateAppearance, resetAppearance,
    workspaceBackground, setWorkspaceBackground, removeWorkspaceBackground,
  } from './appearancePreferences'
  import { appearanceRuntimeError } from './appearanceRuntime'
  import {
    PALETTES, ZOOM_LEVELS, UI_FONTS, CODE_FONTS, CODE_FONT_SIZES,
    resolveFontFamily, type AppearanceSettings,
  } from './appearanceSettings'
  import { appearanceText } from './appearanceI18n'

  const backgrounds = [
    { id: 'pattern', label: 'Default pattern' },
    { id: 'solid', label: 'Solid color' },
    { id: 'image', label: 'Custom image' },
  ] as const
  const fits = [
    { id: 'cover', label: 'Cover' }, { id: 'contain', label: 'Contain' },
    { id: 'center', label: 'Center' }, { id: 'tile', label: 'Tile' },
  ] as const
  const imageAccept = 'image/png,image/jpeg,image/webp,image/gif,image/apng,.png,.jpg,.jpeg,.webp,.gif,.apng'
  let fileInput: HTMLInputElement
  let imageBusy = false
  let saveError = ''
  let imageError = ''
  let restored = false

  $: uiFontFamily = resolveFontFamily($appearancePreferences, 'ui')
  $: codeFontFamily = resolveFontFamily($appearancePreferences, 'code')
  $: backgroundError = imageError || $workspaceBackground.error

  function tr(source: string) { return appearanceText($locale, source) }
  function message(cause: unknown) { return cause instanceof Error ? cause.message : String(cause) }
  function save(patch: Partial<AppearanceSettings>) {
    saveError = ''
    restored = false
    try { updateAppearance(patch) } catch (cause) { saveError = message(cause) }
  }
  function setMode(mode: ThemePreference) {
    saveError = ''
    restored = false
    try { setThemePreference(mode) } catch (cause) { saveError = message(cause) }
  }
  function reset() {
    saveError = ''
    restored = false
    try {
      resetAppearance()
      setThemePreference('system')
      restored = true
    } catch (cause) { saveError = message(cause) }
  }
  async function chooseImage(event: Event) {
    const input = event.currentTarget as HTMLInputElement
    const file = input.files?.[0]
    input.value = ''
    if (!file || imageBusy) return
    imageError = ''
    if (file.size > 16 * 1024 * 1024) {
      imageError = tr('The image is too large. Choose an image smaller than 16 MiB.')
      return
    }
    imageBusy = true
    try { await setWorkspaceBackground(file) }
    catch (cause) { imageError = message(cause) }
    finally { imageBusy = false }
  }
  async function removeImage() {
    if (imageBusy) return
    imageError = ''
    imageBusy = true
    try { await removeWorkspaceBackground() }
    catch (cause) { imageError = message(cause) }
    finally { imageBusy = false }
  }
</script>

<div class="space-y-5">
  {#if saveError}
    <div class="rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-xs" role="alert">
      <strong>{tr('Could not save appearance settings.')}</strong>
      <p class="m-0 mt-1 break-words text-muted-foreground">{tr(saveError)}</p>
    </div>
  {/if}
  {#if $appearanceRuntimeError}
    <div class="rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-xs" role="alert">
      <strong>{tr('Could not apply window zoom.')}</strong>
      <p class="m-0 mt-1 break-words text-muted-foreground">{tr($appearanceRuntimeError)}</p>
    </div>
  {/if}

  <section class="rounded-xl border bg-card p-5 space-y-4" aria-labelledby="appearance-theme-heading">
    <div>
      <h3 id="appearance-theme-heading" class="m-0 flex items-center gap-2 text-sm font-medium"><Palette class="size-4 text-muted-foreground" />{tr('Theme and colors')}</h3>
      <p class="m-0 mt-2 text-xs leading-5 text-muted-foreground">{tr('Choose a light or dark appearance, or follow the operating system.')}</p>
    </div>
    <div class="space-y-2">
      <p class="m-0 text-xs font-medium">{tr('Theme mode')}</p>
      <Select.Root type="single" value={$themePreference} onValueChange={(value) => setMode(value as ThemePreference)}>
        <Select.Trigger class="w-full sm:w-64" aria-label={tr('Theme mode')}>{tr($themePreference === 'system' ? 'System' : $themePreference === 'light' ? 'Light' : 'Dark')}</Select.Trigger>
        <Select.Content>
          <Select.Item value="system" label={tr('System')} />
          <Select.Item value="light" label={tr('Light')} />
          <Select.Item value="dark" label={tr('Dark')} />
        </Select.Content>
      </Select.Root>
    </div>
    <div class="space-y-2">
      <p class="m-0 text-xs font-medium">{tr('Theme palette')}</p>
      <div class="grid grid-cols-2 gap-2 sm:grid-cols-3 xl:grid-cols-4" role="group" aria-label={tr('Theme palette')}>
        {#each PALETTES as palette (palette.id)}
          <button type="button" aria-pressed={$appearancePreferences.palette === palette.id}
            class={['flex min-w-0 items-center gap-2 rounded-md border px-3 py-2 text-left text-xs transition-colors hover:bg-accent focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring', $appearancePreferences.palette === palette.id ? 'border-primary ring-2 ring-primary/25' : 'border-border']}
            onclick={() => save({ palette: palette.id })}>
            <span class="size-4 shrink-0 rounded-full border border-foreground/15" style:background-color={palette.swatch}></span>
            <span class="truncate">{palette.label}</span>
          </button>
        {/each}
      </div>
      <p class="m-0 text-xs leading-5 text-muted-foreground">{tr('Colors apply to buttons, highlights, and workspace surfaces.')}</p>
    </div>
  </section>

  <section class="rounded-xl border bg-card p-5 space-y-4" aria-labelledby="appearance-zoom-heading">
    <div>
      <h3 id="appearance-zoom-heading" class="m-0 flex items-center gap-2 text-sm font-medium"><Scaling class="size-4 text-muted-foreground" />{tr('Window zoom')}</h3>
      <p class="m-0 mt-2 text-xs leading-5 text-muted-foreground">{tr('Resize the whole interface. Changes take effect immediately.')}</p>
    </div>
    <Select.Root type="single" value={String($appearancePreferences.zoom)} onValueChange={(value) => save({ zoom: Number(value) })}>
      <Select.Trigger class="w-full sm:w-64" aria-label={tr('Window zoom')}>{$appearancePreferences.zoom}%</Select.Trigger>
      <Select.Content>
        {#each ZOOM_LEVELS as zoom}<Select.Item value={String(zoom)} label={`${zoom}%${zoom === 100 ? ` (${tr('Default')})` : ''}`} />{/each}
      </Select.Content>
    </Select.Root>
    <p class="m-0 text-xs leading-5 text-muted-foreground">{tr('Use Ctrl/⌘ + or − to zoom, and Ctrl/⌘ 0 to reset.')}</p>
  </section>

  <section class="rounded-xl border bg-card p-5 space-y-4" aria-labelledby="appearance-font-heading">
    <div>
      <h3 id="appearance-font-heading" class="m-0 flex items-center gap-2 text-sm font-medium"><Type class="size-4 text-muted-foreground" />{tr('Fonts')}</h3>
      <p class="m-0 mt-2 text-xs leading-5 text-muted-foreground">{tr('Choose fonts for the interface and code or command details.')}</p>
    </div>
    <div class="space-y-2">
      <p class="m-0 text-xs font-medium">{tr('Interface font')}</p>
      <Select.Root type="single" value={$appearancePreferences.uiFont} onValueChange={(value) => save({ uiFont: value as AppearanceSettings['uiFont'] })}>
        <Select.Trigger class="w-full sm:w-72" aria-label={tr('Interface font')}>{tr(UI_FONTS.find(font => font.id === $appearancePreferences.uiFont)?.label ?? 'System')}</Select.Trigger>
        <Select.Content>
          {#each UI_FONTS as font}<Select.Item value={font.id} label={tr(font.label)} />{/each}
        </Select.Content>
      </Select.Root>
      {#if $appearancePreferences.uiFont === 'custom'}
        <input class="h-9 w-full rounded-md border bg-background px-3 text-xs sm:max-w-96" aria-label={tr('Custom interface font')} placeholder={tr('Installed font family, e.g. Microsoft YaHei')}
          value={$appearancePreferences.uiCustomFont} maxlength="160" onchange={(event) => save({ uiCustomFont: event.currentTarget.value })} />
      {/if}
    </div>
    <div class="space-y-2">
      <p class="m-0 text-xs font-medium">{tr('Code and commands')}</p>
      <div class="flex flex-wrap gap-2">
        <Select.Root type="single" value={$appearancePreferences.codeFont} onValueChange={(value) => save({ codeFont: value as AppearanceSettings['codeFont'] })}>
          <Select.Trigger class="w-full sm:w-72" aria-label={tr('Code and commands')}>{tr(CODE_FONTS.find(font => font.id === $appearancePreferences.codeFont)?.label ?? 'System monospace')}</Select.Trigger>
          <Select.Content>
            {#each CODE_FONTS as font}<Select.Item value={font.id} label={tr(font.label)} />{/each}
          </Select.Content>
        </Select.Root>
        <Select.Root type="single" value={String($appearancePreferences.codeFontSize)} onValueChange={(value) => save({ codeFontSize: Number(value) })}>
          <Select.Trigger class="w-24" aria-label={tr('Code font size')}>{$appearancePreferences.codeFontSize}px</Select.Trigger>
          <Select.Content>
            {#each CODE_FONT_SIZES as size}<Select.Item value={String(size)} label={`${size}px`} />{/each}
          </Select.Content>
        </Select.Root>
      </div>
      {#if $appearancePreferences.codeFont === 'custom'}
        <input class="h-9 w-full rounded-md border bg-background px-3 text-xs sm:max-w-96" aria-label={tr('Custom code font')} placeholder={tr('Installed font family, e.g. Cascadia Code')}
          value={$appearancePreferences.codeCustomFont} maxlength="160" onchange={(event) => save({ codeCustomFont: event.currentTarget.value })} />
      {/if}
      <p class="m-0 text-xs leading-5 text-muted-foreground">{tr('Custom fonts use fonts installed on this device. Unavailable fonts use the system fallback.')}</p>
    </div>
    <div class="min-w-0 rounded-lg border bg-muted/25 p-3" aria-label={tr('Font preview')}>
      <p class="m-0 text-xs font-medium text-muted-foreground">{tr('Font preview')}</p>
      <p class="m-0 mt-3 text-sm leading-6" style:font-family={uiFontFamily}>{tr('Clear words, comfortable reading.')} Aa Bb 0123456789</p>
      <pre class="m-0 mt-2 overflow-x-auto whitespace-pre leading-relaxed" style:font-family={codeFontFamily} style:font-size={`${$appearancePreferences.codeFontSize}px`}>const reply = await agent.run(input)
$ git status --short</pre>
    </div>
  </section>

  <section class="rounded-xl border bg-card p-5 space-y-4" aria-labelledby="appearance-background-heading">
    <div>
      <h3 id="appearance-background-heading" class="m-0 flex items-center gap-2 text-sm font-medium"><Image class="size-4 text-muted-foreground" />{tr('Workspace background')}</h3>
      <p class="m-0 mt-2 text-xs leading-5 text-muted-foreground">{tr('Keep the default pattern, use a plain surface, or choose your own image.')}</p>
    </div>
    <div class="flex flex-wrap gap-2" role="group" aria-label={tr('Workspace background')}>
      {#each backgrounds as background}
        <Button variant={$appearancePreferences.background === background.id ? 'secondary' : 'outline'} aria-pressed={$appearancePreferences.background === background.id} disabled={imageBusy}
          onclick={() => save({ background: background.id })}>{tr(background.label)}</Button>
      {/each}
    </div>
    {#if $appearancePreferences.background === 'image'}
      <div class="space-y-2">
        <div class="flex flex-wrap items-center gap-3">
          <div class="grid h-20 w-32 shrink-0 place-items-center overflow-hidden rounded-lg border bg-muted/30">
            {#if $workspaceBackground.url}
              <img src={$workspaceBackground.url} alt="" class="h-full w-full object-cover" />
            {:else}<Image class="size-6 text-muted-foreground/50" />{/if}
          </div>
          <div class="min-w-0 space-y-2">
            <input bind:this={fileInput} type="file" accept={imageAccept} class="hidden" onchange={(event) => void chooseImage(event)} />
            <div class="flex flex-wrap gap-2">
              <Button variant="outline" disabled={imageBusy || $workspaceBackground.loading} onclick={() => fileInput.click()}>
                {#if imageBusy}<LoaderCircle class="animate-spin" data-icon="inline-start" />{/if}
                {tr($workspaceBackground.url ? 'Replace image' : 'Choose an image')}
              </Button>
              {#if $workspaceBackground.url}
                <Button variant="ghost" disabled={imageBusy || $workspaceBackground.loading} onclick={() => void removeImage()}>{tr('Remove image')}</Button>
              {/if}
            </div>
            <p class="m-0 max-w-64 truncate text-xs text-muted-foreground" title={$workspaceBackground.name ?? ''}>
              {$workspaceBackground.loading ? tr('Loading background image…') : $workspaceBackground.name ?? tr('No background image selected')}
            </p>
          </div>
        </div>
        <p class="m-0 text-xs leading-5 text-muted-foreground">{tr('PNG, JPEG, WebP, or GIF, up to 16 MiB and 40 million pixels. Animated images can play.')}</p>
        {#if backgroundError}
          <div class="rounded-md border border-destructive/30 bg-destructive/5 p-3 text-xs" role="alert">
            <strong>{tr('Could not update the background image.')}</strong>
            <p class="m-0 mt-1 break-words text-muted-foreground">{tr(backgroundError)}</p>
          </div>
        {/if}
      </div>
      <div class="space-y-2">
        <p class="m-0 text-xs font-medium">{tr('Image fit')}</p>
        <Select.Root type="single" value={$appearancePreferences.backgroundFit} onValueChange={(value) => save({ backgroundFit: value as AppearanceSettings['backgroundFit'] })}>
          <Select.Trigger class="w-full sm:w-64" aria-label={tr('Image fit')}>{tr(fits.find(fit => fit.id === $appearancePreferences.backgroundFit)?.label ?? 'Cover')}</Select.Trigger>
          <Select.Content>{#each fits as fit}<Select.Item value={fit.id} label={tr(fit.label)} />{/each}</Select.Content>
        </Select.Root>
      </div>
      <div class="space-y-2">
        <div class="flex items-center justify-between gap-3 text-xs"><label for="appearance-mask" class="font-medium">{tr('Mask opacity')}</label><output for="appearance-mask" class="tabular-nums text-muted-foreground">{$appearancePreferences.backgroundMask}%</output></div>
        <input id="appearance-mask" class="w-full accent-primary" type="range" min="0" max="99" step="1" value={$appearancePreferences.backgroundMask} oninput={(event) => save({ backgroundMask: Number(event.currentTarget.value) })} />
        <p class="m-0 text-xs leading-5 text-muted-foreground">{tr('A stronger mask fades the image toward the theme background and improves text contrast.')}</p>
      </div>
      <div class="space-y-2">
        <div class="flex items-center justify-between gap-3 text-xs"><label for="appearance-blur" class="font-medium">{tr('Image blur')}</label><output for="appearance-blur" class="tabular-nums text-muted-foreground">{$appearancePreferences.backgroundBlur}px</output></div>
        <input id="appearance-blur" class="w-full accent-primary" type="range" min="0" max="24" step="1" value={$appearancePreferences.backgroundBlur} oninput={(event) => save({ backgroundBlur: Number(event.currentTarget.value) })} />
      </div>
      <div class="space-y-2">
        <div class="flex items-center justify-between gap-3 text-xs"><label for="appearance-panel-opacity" class="font-medium">{tr('Panel opacity')}</label><output for="appearance-panel-opacity" class="tabular-nums text-muted-foreground">{$appearancePreferences.panelOpacity}%</output></div>
        <input id="appearance-panel-opacity" class="w-full accent-primary" type="range" min="0" max="100" step="1" value={$appearancePreferences.panelOpacity} oninput={(event) => save({ panelOpacity: Number(event.currentTarget.value) })} />
        <p class="m-0 text-xs leading-5 text-muted-foreground">{tr('Lower values let more of the image show through workspace panels.')}</p>
      </div>
    {/if}
  </section>

  <div class="space-y-2 border-t pt-4">
    <Button variant="outline" disabled={imageBusy} onclick={reset}><RotateCcw data-icon="inline-start" />{tr('Restore appearance defaults')}</Button>
    <p class="m-0 text-xs leading-5 text-muted-foreground">{tr('Restores the theme, colors, zoom, fonts, and default pattern. Your uploaded image is kept.')}</p>
    {#if restored}<p class="m-0 text-xs text-success" role="status">{tr('Appearance defaults restored.')}</p>{/if}
  </div>
</div>

<script lang="ts">
  import { Download, ExternalLink, RotateCw } from '@lucide/svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import { createUnavailableWorkbenchCapabilities } from '$lib/capabilities/unavailableCapabilities'
  import type { CapabilitySlot, UpdaterCapability } from '$lib/capabilities/workbenchCapabilities'
  import * as Dialog from '$lib/components/ui/dialog'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import {
    canInstallInAppUpdate,
    dismissUpdateDialog,
    updateDialogOpen,
    updateState,
  } from '$lib/updater'

  export let installBlocked = false
  export let onOpenReleases: () => void = () => {}
  export let softwareUpdates: CapabilitySlot<UpdaterCapability> =
    createUnavailableWorkbenchCapabilities().softwareUpdates

  const canInstall = canInstallInAppUpdate()

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  $: progress =
    $updateState.total > 0
      ? Math.min(100, Math.round(($updateState.downloaded / $updateState.total) * 100))
      : 0

  $: notes = $updateState.message.trim()
  $: busy = $updateState.status === 'downloading'

  function handleOpenChange(open: boolean) {
    if (open) updateDialogOpen.set(true)
    else dismissUpdateDialog()
  }
</script>

<Dialog.Root open={$updateDialogOpen} onOpenChange={handleOpenChange}>
  <!-- The update prompt is a system-level modal: it must always stack above
       every regular dialog (z-[110]) and popover (z-[130]) no matter when it
       opened relative to them, so e.g. a taller Settings dialog never covers
       or hides it. Toasts stay above everything. -->
  <Dialog.Content
    class="max-w-lg gap-5 sm:max-w-lg z-[140]"
    overlayClass="z-[140]"
    showCloseButton={!busy}
    interactOutsideBehavior={busy ? 'ignore' : 'close'}
    escapeKeydownBehavior={busy ? 'ignore' : 'close'}
  >
    <Dialog.Header>
      <div class="mb-1 flex items-center gap-2">
        <Badge variant="secondary">{tr('Software updates')}</Badge>
        {#if $updateState.version}
          <Badge variant="outline">v{$updateState.version}</Badge>
        {/if}
      </div>
      <Dialog.Title>{tr('A new version of RambleDesk is available.')}</Dialog.Title>
      <Dialog.Description class="leading-5">
        {#if $updateState.status === 'ready'}
          {tr('v{version} is installed and will take effect after restart.', {
            version: $updateState.version,
          })}
        {:else if $updateState.status === 'downloading'}
          {tr('Downloading v{version}…', { version: $updateState.version })}
        {:else}
          {tr('Version v{version} is available', { version: $updateState.version })}
        {/if}
      </Dialog.Description>
    </Dialog.Header>

    <section class="grid gap-2">
      <h3 class="m-0 text-xs font-medium">{tr("What's new")}</h3>
      <div class="max-h-56 overflow-auto rounded-lg border bg-muted/25 px-3 py-3">
        {#if notes && $updateState.status !== 'error'}
          <pre class="m-0 whitespace-pre-wrap font-sans text-xs leading-5 text-foreground">{notes}</pre>
        {:else if $updateState.status === 'error'}
          <p class="m-0 text-xs leading-5 text-destructive">{$updateState.message}</p>
        {:else}
          <p class="m-0 text-xs leading-5 text-muted-foreground">
            {tr('Release notes are not available for this version.')}
          </p>
        {/if}
      </div>
    </section>

    {#if $updateState.status === 'downloading'}
      <div>
        <div class="flex items-center justify-between gap-3 text-xs">
          <span>{tr('Downloading v{version}…', { version: $updateState.version })}</span>
          {#if $updateState.total > 0}<span>{progress}%</span>{/if}
        </div>
        <div class="mt-3 h-2 overflow-hidden rounded-full bg-muted">
          <div
            class={['h-full bg-primary transition-[width]', $updateState.total <= 0 ? 'animate-pulse' : '']}
            style={`width: ${$updateState.total > 0 ? progress : 35}%`}
          ></div>
        </div>
      </div>
    {/if}

    {#if canInstall && installBlocked && ($updateState.status === 'available' || $updateState.status === 'ready')}
      <p class="m-0 text-xs leading-5 text-warning-foreground dark:text-warning">
        {tr('Feedback is in progress or has unsaved content. Update restart is disabled to prevent data loss.')}
      </p>
    {/if}

    <Dialog.Footer>
      {#if !busy}
        <Button variant="outline" onclick={() => dismissUpdateDialog()}>{tr('Later')}</Button>
      {/if}
      {#if $updateState.status === 'ready'}
        <Button
          disabled={installBlocked}
          title={installBlocked ? tr('Finish or cancel the current feedback before restarting.') : ''}
          onclick={() => void softwareUpdates.implementation.restart()}
        >
          <RotateCw data-icon="inline-start" />
          {tr('Restart now')}
        </Button>
      {:else if canInstall}
        <Button
          disabled={installBlocked || busy}
          title={installBlocked ? tr('Finish or cancel the current feedback before installing the update.') : ''}
          onclick={() => void softwareUpdates.implementation.install()}
        >
          <Download data-icon="inline-start" />
          {tr('Download and install')}
        </Button>
      {:else}
        <Button
          onclick={() => {
            onOpenReleases()
            dismissUpdateDialog()
          }}
        >
          <ExternalLink data-icon="inline-start" />
          {tr('Open GitHub Releases')}
        </Button>
      {/if}
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

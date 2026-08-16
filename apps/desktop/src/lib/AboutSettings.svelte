<script lang="ts">
  import { getVersion } from '@tauri-apps/api/app'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { Download, ExternalLink, GitBranch, LoaderCircle, RefreshCw, RotateCw, ShieldCheck, Sparkles } from '@lucide/svelte'
  import { onMount } from 'svelte'

  import rambelleSticker from '../assets/rambelle-states/idle.png'
  import RambelleProfileDialog from './RambelleProfileDialog.svelte'
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import {
    checkForUpdates,
    downloadAndInstallUpdate,
    restartAfterUpdate,
    updateState,
  } from '$lib/updater'

  export let installBlocked = false

  let version = '0.0.1'
  let profileOpen = false
  const isTauri = '__TAURI_INTERNALS__' in window
  const projectUrl = 'https://github.com/l1veIn/rambledesk'

  onMount(async () => {
    if (isTauri) version = await getVersion().catch(() => version)
  })

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  async function openProject() {
    if (isTauri) {
      await openUrl(projectUrl)
      return
    }
    window.open(projectUrl, '_blank', 'noopener,noreferrer')
  }

  $: progress =
    $updateState.total > 0
      ? Math.min(100, Math.round(($updateState.downloaded / $updateState.total) * 100))
      : 0
</script>

<div class="space-y-6">
  <section class="relative overflow-hidden rounded-xl border bg-gradient-to-br from-primary/8 via-background to-info/8 p-6">
    <div class="relative z-10 grid grid-cols-[minmax(0,1fr)_150px] items-center gap-6">
      <div>
        <div class="flex flex-wrap items-center gap-2">
          <h3 class="m-0 text-xl font-semibold tracking-tight">RambleDesk</h3>
          <Badge variant="secondary">v{version}</Badge>
          <Badge variant="outline">Windows</Badge>
        </div>
        <p class="m-0 mt-3 max-w-xl text-sm leading-6 text-muted-foreground">
          {tr('Let agents pause at key moments and request structured human feedback that can be resumed and archived.')}
        </p>
        <p class="m-0 mt-2 text-xs leading-5 text-muted-foreground">
          {tr('Feedback drafts, attachments, and packages stay on your device. The agent only receives results you explicitly submit or cancel.')}
        </p>
        <Button variant="link" class="mt-3 h-auto gap-1.5 p-0 text-xs" onclick={() => void openProject()}>
          <GitBranch data-icon="inline-start" />
          {tr('View GitHub repository')}
          <ExternalLink class="size-3" />
        </Button>
      </div>
      <img
        src={rambelleSticker}
        alt={tr('Rambelle waving sticker')}
        class="mx-auto h-36 w-36 object-contain drop-shadow-[0_16px_30px_rgba(59,130,246,0.2)]"
      />
    </div>
  </section>

  <section class="rounded-xl border p-5">
    <div class="flex items-center justify-between gap-6">
      <div class="flex min-w-0 items-center gap-3">
        <img
          src={rambelleSticker}
          alt=""
          draggable="false"
          class="size-12 shrink-0 rounded-xl object-contain"
        />
        <div class="min-w-0">
          <h3 class="m-0 text-sm font-medium">Rambelle</h3>
          <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
            {tr('View Rambelle’s story and character profile.')}
          </p>
        </div>
      </div>
      <Button variant="outline" class="shrink-0" onclick={() => (profileOpen = true)}>
        <Sparkles data-icon="inline-start" />
        {tr('View character profile')}
      </Button>
    </div>
  </section>

  <section class="rounded-xl border p-5">
    <div class="flex items-start justify-between gap-6">
      <div>
        <h3 class="m-0 text-sm font-medium">{tr('Software updates')}</h3>
        <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
          {tr('RambleDesk checks for updates quietly after launch, and you can check manually at any time.')}
        </p>
      </div>
      <Button
        variant="outline"
        disabled={!isTauri || $updateState.status === 'checking' || $updateState.status === 'downloading'}
        onclick={() => void checkForUpdates()}
      >
        {#if $updateState.status === 'checking'}
          <LoaderCircle class="animate-spin" data-icon="inline-start" />
          {tr('Checking…')}
        {:else}
          <RefreshCw data-icon="inline-start" />
          {tr('Check for updates')}
        {/if}
      </Button>
    </div>

    <div class="mt-4 rounded-lg border bg-muted/25 p-4" aria-live="polite">
      {#if $updateState.status === 'idle'}
        <p class="m-0 text-xs text-muted-foreground">{tr('Updates have not been checked yet.')}</p>
      {:else if $updateState.status === 'checking'}
        <p class="m-0 text-xs text-muted-foreground">{tr('Connecting to the update server…')}</p>
      {:else if $updateState.status === 'up-to-date'}
        <div class="flex items-center gap-2 text-xs text-success">
          <ShieldCheck class="size-4" />
          {tr('You are up to date.')}
        </div>
      {:else if $updateState.status === 'available'}
        <div class="flex flex-wrap items-center justify-between gap-3">
          <div>
            <strong class="block text-xs">{tr('Version v{version} is available', { version: $updateState.version })}</strong>
            {#if $updateState.message}
              <p class="m-0 mt-1 line-clamp-3 text-[10px] leading-4 text-muted-foreground">{$updateState.message}</p>
            {/if}
          </div>
          <Button
            disabled={installBlocked}
            title={installBlocked ? tr('Finish or cancel the current feedback before installing the update.') : ''}
            onclick={() => void downloadAndInstallUpdate()}
          >
            <Download data-icon="inline-start" />
            {tr('Download and install')}
          </Button>
        </div>
      {:else if $updateState.status === 'downloading'}
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
      {:else if $updateState.status === 'ready'}
        <div class="flex flex-wrap items-center justify-between gap-3">
          <strong class="text-xs">{tr('v{version} is installed and will take effect after restart.', { version: $updateState.version })}</strong>
          <Button
            disabled={installBlocked}
            title={installBlocked ? tr('Finish or cancel the current feedback before restarting.') : ''}
            onclick={() => void restartAfterUpdate()}
          >
            <RotateCw data-icon="inline-start" />
            {tr('Restart now')}
          </Button>
        </div>
      {:else if $updateState.status === 'error'}
        <div>
          <strong class="block text-xs text-destructive">{tr('Update check or installation failed')}</strong>
          <p class="m-0 mt-1 break-all text-[10px] leading-4 text-muted-foreground">{$updateState.message}</p>
        </div>
      {/if}
    </div>

    {#if installBlocked && ($updateState.status === 'available' || $updateState.status === 'ready')}
      <p class="m-0 mt-3 text-[10px] leading-4 text-warning-foreground dark:text-warning">
        {tr('Feedback is in progress or has unsaved content. Update restart is disabled to prevent data loss.')}
      </p>
    {/if}
  </section>

  <p class="m-0 text-center text-[10px] text-muted-foreground">
    © 2026 RambleDesk · MIT · {tr('See THIRD_PARTY_NOTICES.md for third-party component notices')}
  </p>
</div>

<RambelleProfileDialog bind:open={profileOpen} />

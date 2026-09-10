<script lang="ts">
  import { onMount } from 'svelte'
  import { Clipboard, Globe2, LoaderCircle, Play, RefreshCw, RotateCcw, X } from '@lucide/svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import { toast } from '$lib/components/ui/sonner'
  import {
    settleWebAccessMutation,
    webAccessDisplayState as resolveWebAccessDisplayState,
    webAccessRunningActionsEnabled,
    webAccessToggleTarget,
  } from '$lib/capabilities/webAccessState'
  import type {
    WebAccessStatus,
    WorkbenchCapabilities,
  } from '$lib/capabilities/workbenchCapabilities'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import {
    DEFAULT_WEB_ACCESS_PORT,
    WEB_ACCESS_PORT_MAX,
    WEB_ACCESS_PORT_MIN,
    initialWebAccessAutostart,
    initialWebAccessPort,
    normalizeWebAccessPort,
    saveWebAccessAutostart,
    saveWebAccessPort,
  } from '$lib/uiPreferences'

  export let capabilities: WorkbenchCapabilities

  type WebAccessTransientPhase = 'loading' | 'starting' | 'stopping' | null

  let webAccessStatus: WebAccessStatus | null = null
  let webAccessPhase: WebAccessTransientPhase = 'loading'
  let webAccessActionError = ''
  let webAccessRefreshError = ''
  let port = initialWebAccessPort()
  let portDraft = String(port)
  let autostart = initialWebAccessAutostart()
  let rotating = false
  let confirmingRotate = false

  $: webAccessDisplayState = resolveWebAccessDisplayState(webAccessStatus, webAccessPhase)
  $: webAccessRunningActions = webAccessRunningActionsEnabled(webAccessStatus, webAccessPhase)
  $: webAccessError = webAccessStatus?.state === 'failed'
    ? webAccessStatus.failure.message
    : webAccessRefreshError || webAccessActionError
  $: activePort = portFromUrl(webAccessStatus?.state === 'running' ? webAccessStatus.url : null)
  $: portPendingRestart =
    webAccessStatus?.state === 'running' && activePort !== null && activePort !== port

  onMount(() => {
    void refreshWebAccessStatus()
  })

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function messageFrom(cause: unknown) {
    if (cause instanceof Error) return cause.message
    if (cause && typeof cause === 'object' && 'message' in cause) {
      return String((cause as { message: unknown }).message)
    }
    return String(cause)
  }

  function portFromUrl(url: string | null | undefined): number | null {
    if (!url) return null
    const match = /:(\d+)(?:\/|$)/u.exec(url)
    return match ? Number(match[1]) : null
  }

  async function refreshWebAccessStatus() {
    if (capabilities.webAccessAdministration.status.availability === 'unavailable') return
    webAccessPhase = 'loading'
    webAccessActionError = ''
    webAccessRefreshError = ''
    try {
      webAccessStatus = await capabilities.webAccessAdministration.implementation.status()
    } catch (cause) {
      webAccessStatus = null
      webAccessRefreshError = messageFrom(cause)
    } finally {
      webAccessPhase = null
    }
  }

  async function toggleWebAccess() {
    if (
      capabilities.webAccessAdministration.status.availability === 'unavailable' ||
      webAccessPhase !== null
    ) return
    const enabled = webAccessToggleTarget(webAccessStatus)
    if (enabled === null) return
    webAccessPhase = enabled ? 'starting' : 'stopping'
    webAccessActionError = ''
    webAccessRefreshError = ''
    const result = await settleWebAccessMutation(
      capabilities.webAccessAdministration.implementation,
      enabled,
      enabled ? port : undefined,
    )
    webAccessStatus = result.status
    webAccessPhase = null
    webAccessActionError = result.operationError ? messageFrom(result.operationError) : ''
    webAccessRefreshError = result.refreshError
      ? tr('Could not verify the current Web Access status: {error}', {
          error: messageFrom(result.refreshError),
        })
      : ''
  }

  async function openWebAccess() {
    if (!webAccessRunningActions) return
    try {
      await capabilities.webAccessAdministration.implementation.open()
    } catch (cause) {
      await refreshWebAccessAfterActionFailure(cause)
    }
  }

  async function copyWebAccessToken() {
    if (!webAccessRunningActions) return
    try {
      await capabilities.webAccessAdministration.implementation.copyToken()
      toast.success(tr('Web Access token copied.'))
    } catch (cause) {
      await refreshWebAccessAfterActionFailure(cause)
    }
  }

  async function refreshWebAccessAfterActionFailure(actionCause: unknown) {
    const actionError = messageFrom(actionCause)
    webAccessPhase = 'loading'
    webAccessRefreshError = ''
    try {
      webAccessStatus = await capabilities.webAccessAdministration.implementation.status()
      webAccessActionError = webAccessStatus.state === 'failed' ? '' : actionError
    } catch (refreshCause) {
      webAccessStatus = null
      webAccessActionError = actionError
      webAccessRefreshError = tr('Could not verify the current Web Access status: {error}', {
        error: messageFrom(refreshCause),
      })
    } finally {
      webAccessPhase = null
    }
  }

  function commitPort() {
    const next = Number(portDraft.trim())
    if (
      !Number.isInteger(next) ||
      next < WEB_ACCESS_PORT_MIN ||
      next > WEB_ACCESS_PORT_MAX
    ) {
      portDraft = String(port)
      return
    }
    port = normalizeWebAccessPort(next)
    portDraft = String(port)
    saveWebAccessPort(port)
  }

  function toggleAutostart() {
    autostart = !autostart
    saveWebAccessAutostart(autostart)
  }

  async function rotateToken() {
    if (rotating) return
    if (!confirmingRotate) {
      confirmingRotate = true
      return
    }
    confirmingRotate = false
    rotating = true
    webAccessActionError = ''
    try {
      webAccessStatus = await capabilities.webAccessAdministration.implementation.rotateToken()
      toast.success(tr('A new Web Access token was issued. Every browser must enter it again.'))
    } catch (cause) {
      await refreshWebAccessAfterActionFailure(cause)
    } finally {
      rotating = false
    }
  }
</script>

<div class="space-y-8">
  <section class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-8 border-b pb-8">
    <div class="flex gap-3">
      <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
        <Globe2 class="size-4" />
      </span>
      <div>
        <div class="flex items-center gap-2">
          <h3 class="m-0 text-sm font-medium">{tr('Web Access')}</h3>
          <Badge
            variant={webAccessDisplayState === 'failed'
              ? 'destructive'
              : webAccessDisplayState === 'running'
                ? 'default'
                : 'secondary'}
          >
            {webAccessDisplayState === 'loading'
              ? tr('Loading…')
              : webAccessDisplayState === 'starting'
                ? tr('Starting…')
                : webAccessDisplayState === 'stopping'
                  ? tr('Stopping…')
                  : webAccessDisplayState === 'running'
                    ? tr('Running')
                    : webAccessDisplayState === 'stopped'
                      ? tr('Stopped')
                      : webAccessDisplayState === 'failed'
                        ? tr('Needs attention')
                        : tr('Status unavailable')}
          </Badge>
        </div>
        <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
          {webAccessStatus?.state === 'running'
            ? tr('Available only in a browser on this computer at {url}.', {
                url: webAccessStatus.url,
              })
            : webAccessStatus?.state === 'failed'
              ? tr('Web Access is unavailable until the reported problem is resolved.')
              : webAccessStatus?.state === 'stopped'
                ? tr('Start a local browser Workbench. It stays off until you start it.')
                : tr('The current Web Access status could not be verified.')}
        </p>
        <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
          {tr('Stopping Web Access only closes browser access. Backend Runtime and Local Integration keep running.')}
        </p>
        {#if webAccessError}
          <p class="m-0 mt-1 text-xs text-destructive" role="alert">{webAccessError}</p>
        {/if}
      </div>
    </div>
    <div class="flex flex-wrap items-center justify-end gap-2">
      <Button
        variant="ghost"
        disabled={webAccessPhase !== null}
        onclick={() => void refreshWebAccessStatus()}
      >
        <RefreshCw
          data-icon="inline-start"
          class={webAccessPhase === 'loading' ? 'animate-spin' : ''}
        />
        {tr('Refresh')}
      </Button>
      {#if webAccessRunningActions}
        <Button variant="outline" onclick={() => void copyWebAccessToken()}>
          <Clipboard data-icon="inline-start" />
          {tr('Copy token')}
        </Button>
        <Button variant="outline" onclick={() => void openWebAccess()}>
          <Globe2 data-icon="inline-start" />
          {tr('Open')}
        </Button>
      {/if}
      <Button
        variant={webAccessStatus?.state === 'running' ? 'destructive' : 'outline'}
        disabled={webAccessPhase !== null || webAccessStatus === null}
        onclick={() => void toggleWebAccess()}
      >
        {#if webAccessPhase !== null}
          <LoaderCircle data-icon="inline-start" class="animate-spin" />
        {:else if webAccessStatus?.state === 'running'}
          <X data-icon="inline-start" />
        {:else}
          <Play data-icon="inline-start" />
        {/if}
        {webAccessPhase === 'loading'
          ? tr('Loading…')
          : webAccessPhase === 'starting'
            ? tr('Starting…')
            : webAccessPhase === 'stopping'
              ? tr('Stopping…')
              : webAccessStatus?.state === 'running'
                ? tr('Stop')
                : tr('Start')}
      </Button>
    </div>
  </section>

  <section class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-8 border-b pb-8">
    <div class="flex gap-3">
      <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
        <Globe2 class="size-4" />
      </span>
      <div>
        <h3 class="m-0 text-sm font-medium">{tr('Browser server')}</h3>
        <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
          {tr('The browser Workbench listens on this loopback port. Changes apply the next time Web Access starts.')}
        </p>
        {#if portPendingRestart}
          <p class="m-0 mt-1 text-xs text-warning-foreground dark:text-warning" role="status">
            {tr('Restart Web Access to use port {port}.', { port })}
          </p>
        {/if}
      </div>
    </div>
    <div class="flex flex-wrap items-center justify-end gap-2">
      <label class="flex items-center gap-2 text-xs text-muted-foreground" for="web-access-port">
        {tr('Port')}
      </label>
      <input
        id="web-access-port"
        class="h-8 w-24 rounded-md border bg-background px-2 text-right text-xs tabular-nums outline-none focus-visible:ring-2 focus-visible:ring-ring"
        inputmode="numeric"
        autocomplete="off"
        value={portDraft}
        aria-describedby="web-access-port-range"
        oninput={(event) => (portDraft = event.currentTarget.value)}
        onblur={commitPort}
        onkeydown={(event) => {
          if (event.key === 'Enter') {
            event.preventDefault()
            commitPort()
            event.currentTarget.blur()
          }
        }}
      />
      <span id="web-access-port-range" class="sr-only">
        {tr('Ports {min} to {max}', {
          min: WEB_ACCESS_PORT_MIN,
          max: WEB_ACCESS_PORT_MAX,
        })}
      </span>
      <Button
        variant="ghost"
        disabled={port === DEFAULT_WEB_ACCESS_PORT}
        onclick={() => {
          port = DEFAULT_WEB_ACCESS_PORT
          portDraft = String(DEFAULT_WEB_ACCESS_PORT)
          saveWebAccessPort(port)
        }}
      >
        {tr('Reset')}
      </Button>
    </div>
  </section>

  <section class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-8 border-b pb-8">
    <div class="flex gap-3">
      <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
        <Play class="size-4" />
      </span>
      <div>
        <h3 class="m-0 text-sm font-medium">{tr('Start with RambleDesk')}</h3>
        <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
          {tr('Start Web Access automatically when the desktop app opens.')}
        </p>
      </div>
    </div>
    <Button
      variant={autostart ? 'default' : 'outline'}
      aria-pressed={autostart}
      onclick={toggleAutostart}
    >
      {autostart ? tr('On') : tr('Off')}
    </Button>
  </section>

  <section class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-8">
    <div class="flex gap-3">
      <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
        <RotateCcw class="size-4" />
      </span>
      <div>
        <h3 class="m-0 text-sm font-medium">{tr('Refresh access token')}</h3>
        <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
          {tr('Issues a new token and signs every browser out. Copy the new token before using Web Access again.')}
        </p>
      </div>
    </div>
    <div class="flex flex-wrap items-center justify-end gap-2">
      {#if confirmingRotate}
        <Button variant="ghost" onclick={() => (confirmingRotate = false)}>
          {tr('Cancel')}
        </Button>
      {/if}
      <Button
        variant={confirmingRotate ? 'destructive' : 'outline'}
        disabled={rotating}
        onclick={() => void rotateToken()}
      >
        {#if rotating}
          <LoaderCircle data-icon="inline-start" class="animate-spin" />
        {:else}
          <RotateCcw data-icon="inline-start" />
        {/if}
        {confirmingRotate ? tr('Confirm refresh') : tr('Refresh token')}
      </Button>
    </div>
  </section>
</div>

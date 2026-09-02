<script lang="ts">
  import { Bell, BellOff, Copy, Minus, Square, X } from '@lucide/svelte'
  import { onMount, type Snippet } from 'svelte'

  import appIcon from '../assets/rambledesk-app-icon.webp'
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import { createUnavailableWorkbenchCapabilities } from '$lib/capabilities/unavailableCapabilities'
  import type {
    CapabilitySlot,
    NotificationCapability,
    WindowCapability,
  } from '$lib/capabilities/workbenchCapabilities'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'

  const unavailableCapabilities = createUnavailableWorkbenchCapabilities()

  export let sidebarCollapsed = false
  export let workspaceTabs: Snippet<[
    onStartDragging: ((event: PointerEvent) => void) | null,
  ]>
  export let pendingCount = 0
  export let rambleEngaged = false
  export let rambleActive = false
  export let rambleRequestTitle = ''
  export let notificationText = ''
  export let notificationEnabled = false
  export let notificationDisabled = false
  export let windowControls: CapabilitySlot<WindowCapability> = unavailableCapabilities.windowControls
  export let notifications: CapabilitySlot<NotificationCapability> = unavailableCapabilities.notifications
  export let onNotifications: () => void = () => {}
  export let onWindowError: (message: string) => void = () => {}

  $: windowControlsAvailable = windowControls.status.availability !== 'unavailable'
  $: notificationsAvailable = notifications.status.availability !== 'unavailable'
  $: isMac = windowControls.implementation.platform() === 'macOS'
  let maximized = false

  onMount(() => {
    if (!windowControlsAvailable) return
    void refreshMaximized()
    return windowControls.implementation.onResized(
      () => void refreshMaximized(),
      (cause) => onWindowError(cause instanceof Error ? cause.message : String(cause)),
    )
  })

  async function refreshMaximized() {
    if (isMac) return
    maximized = await windowControls.implementation.isMaximized()
  }

  async function runWindowAction(action: 'minimize' | 'maximize' | 'close') {
    if (!windowControlsAvailable) return
    try {
      if (action === 'minimize') await windowControls.implementation.minimize()
      else if (action === 'maximize') {
        if (isMac) return
        await windowControls.implementation.toggleMaximize()
        maximized = await windowControls.implementation.isMaximized()
      } else await windowControls.implementation.close()
    } catch (cause) {
      onWindowError(cause instanceof Error ? cause.message : String(cause))
    }
  }

  async function startDragging(event: PointerEvent) {
    if (!windowControlsAvailable || event.button !== 0) return
    const target = event.target
    if (target instanceof Element) {
      const button = target.closest('button')
      if (button && !button.matches('.titlebar-brand, .titlebar-drag')) return
    }
    try {
      await windowControls.implementation.startDragging()
    } catch (cause) {
      onWindowError(cause instanceof Error ? cause.message : String(cause))
    }
  }
</script>

<header
  class={[
    'relative z-30 flex h-[46px] select-none items-stretch rounded-t-[15px] border-b bg-background/95 backdrop-blur-md',
  ]}
>
  {#if windowControlsAvailable && isMac}
    <div class="absolute left-[15px] flex h-full items-center gap-2" aria-label={t($locale, 'Window controls')}>
      <button
        class="traffic close size-3 rounded-full border border-black/10"
        aria-label={t($locale, 'Close window')}
        onclick={() => runWindowAction('close')}
      ></button>
      <button
        class="traffic minimize size-3 rounded-full border border-black/10"
        aria-label={t($locale, 'Minimize window')}
        onclick={() => runWindowAction('minimize')}
      ></button>
    </div>
  {/if}

  <div
    class={[
      'flex h-full shrink-0 items-stretch border-r border-sidebar-border bg-sidebar text-sidebar-foreground transition-[width] duration-200',
      sidebarCollapsed ? 'w-14' : 'w-[224px]',
    ]}
  >
    {#if windowControlsAvailable}
      <button
        class={[
          'titlebar-brand flex h-full min-w-0 flex-1 cursor-grab items-center border-0 bg-transparent text-left active:cursor-grabbing focus-visible:outline-2 focus-visible:outline-offset-[-3px] focus-visible:outline-ring',
          sidebarCollapsed ? 'justify-center px-2' : 'gap-2.5 px-3',
          isMac && !sidebarCollapsed ? 'pl-[58px]' : '',
        ]}
        aria-label={t($locale, 'Drag window')}
        title={t($locale, 'Drag window')}
        onpointerdown={(event) => void startDragging(event)}
      >
        {#if !sidebarCollapsed || !isMac}
          <img class="size-7 shrink-0 rounded-md object-contain" src={appIcon} alt="" draggable="false" />
        {/if}
        {#if !sidebarCollapsed}
          <strong class="truncate text-xs font-semibold">RambleDesk</strong>
        {/if}
      </button>
    {:else}
      <div
        class={[
          'titlebar-brand flex h-full min-w-0 flex-1 items-center text-sidebar-foreground',
          sidebarCollapsed ? 'justify-center px-2' : 'gap-2.5 px-3',
        ]}
      >
        <img class="size-7 shrink-0 rounded-md object-contain" src={appIcon} alt="" draggable="false" />
        {#if !sidebarCollapsed}
          <strong class="truncate text-xs font-semibold">RambleDesk</strong>
        {/if}
      </div>
    {/if}
  </div>

  <div class="flex min-w-0 flex-1 items-stretch bg-muted/25">
    <div class="min-w-0 flex-1">
      {@render workspaceTabs(windowControlsAvailable ? startDragging : null)}
    </div>

    <div class="flex shrink-0 items-center gap-1.5 px-2">
      {#if rambleEngaged}
        <Badge
          variant="secondary"
          class={[
            'h-6 max-w-64 gap-1.5 px-2 text-[9px] max-[1080px]:max-w-40',
            rambleActive
              ? 'bg-destructive/10 text-destructive'
              : 'bg-warning/10 text-warning-foreground dark:text-warning',
          ]}
          title={rambleRequestTitle}
        >
          <span
            class={[
              'size-1.5 shrink-0 rounded-full',
              rambleActive ? 'animate-pulse bg-destructive' : 'bg-warning',
            ]}
          ></span>
          <span class="truncate">
            {rambleActive ? t($locale, 'Recording') : t($locale, 'Ramble paused')} · {rambleRequestTitle}
          </span>
        </Badge>
      {/if}
      {#if pendingCount > 0}
        <Badge
          variant="secondary"
          class="h-6 bg-warning/10 px-2 text-[9px] text-warning-foreground max-[900px]:hidden dark:text-warning"
        >
          {pendingCount} {t($locale, 'pending')}
        </Badge>
      {/if}
      {#if notificationsAvailable}
        <Button
          variant="ghost"
          size="icon"
          class={notificationEnabled ? 'text-info' : ''}
          disabled={notificationDisabled}
          onclick={onNotifications}
          title={notificationText || t($locale, 'Notifications')}
          aria-label={notificationText || t($locale, 'Notifications')}
        >
          {#if notificationEnabled}<Bell />{:else}<BellOff />{/if}
        </Button>
      {/if}
    </div>

    {#if windowControlsAvailable && !isMac}
      <div class="ml-1 flex items-stretch" aria-label={t($locale, 'Window controls')}>
        <button
          class="grid w-11 place-items-center text-muted-foreground hover:bg-muted hover:text-foreground"
          aria-label={t($locale, 'Minimize window')}
          onclick={() => runWindowAction('minimize')}
        >
          <Minus class="size-4" />
        </button>
        <button
          class="grid w-11 place-items-center text-muted-foreground hover:bg-muted hover:text-foreground"
          aria-label={t($locale, 'Maximize or restore window')}
          onclick={() => runWindowAction('maximize')}
        >
          {#if maximized}<Copy class="size-3.5" />{:else}<Square class="size-3.5" />{/if}
        </button>
        <button
          class="grid w-11 place-items-center text-muted-foreground hover:bg-destructive hover:text-white"
          aria-label={t($locale, 'Close window')}
          onclick={() => runWindowAction('close')}
        >
          <X class="size-4" />
        </button>
      </div>
    {/if}
  </div>
</header>

<style>
  .traffic.close {
    background: #ff5f57;
  }

  .traffic.minimize {
    background: #febc2e;
  }
</style>

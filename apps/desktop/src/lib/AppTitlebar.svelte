<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { Bell, BellOff, Copy, Minus, Square, X } from '@lucide/svelte'
  import { onMount } from 'svelte'

  import appIcon from '../assets/rambledesk-app-icon.webp'
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import { currentDesktopPlatform } from '$lib/platform'

  export let sourceLabel = 'Workbench'
  export let pendingCount = 0
  export let rambleEngaged = false
  export let rambleActive = false
  export let rambleRequestTitle = ''
  export let notificationText = ''
  export let notificationEnabled = false
  export let notificationDisabled = false
  export let onNotifications: () => void = () => {}
  export let onWindowError: (message: string) => void = () => {}

  const isTauri = '__TAURI_INTERNALS__' in window
  const isMac = currentDesktopPlatform() === 'macOS'
  let maximized = false

  onMount(() => {
    if (!isTauri) return
    const appWindow = getCurrentWindow()
    void appWindow.isMaximized().then((value) => {
      maximized = value
    })
    const unlisten = appWindow.onResized(() => {
      void appWindow.isMaximized().then((value) => {
        maximized = value
      })
    })
    return () => {
      void unlisten.then((dispose) => dispose())
    }
  })

  async function runWindowAction(action: 'minimize' | 'maximize' | 'close') {
    if (!isTauri) return
    try {
      const appWindow = getCurrentWindow()
      if (action === 'minimize') await appWindow.minimize()
      else if (action === 'maximize') {
        await appWindow.toggleMaximize()
        maximized = await appWindow.isMaximized()
      } else await appWindow.close()
    } catch (cause) {
      onWindowError(cause instanceof Error ? cause.message : String(cause))
    }
  }

  async function startDragging(event: PointerEvent) {
    if (!isTauri || event.button !== 0) return
    const target = event.target
    if (target instanceof Element) {
      const button = target.closest('button')
      if (button && !button.matches('.titlebar-brand, .titlebar-drag')) return
    }
    try {
      await getCurrentWindow().startDragging()
    } catch (cause) {
      onWindowError(cause instanceof Error ? cause.message : String(cause))
    }
  }
</script>

<header
  class={[
    'relative z-30 flex h-[46px] select-none items-stretch rounded-t-[15px] border-b bg-background/95 backdrop-blur-md',
    isMac ? 'pl-[78px]' : '',
  ]}
>
  {#if isMac}
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
      <button
        class="traffic maximize size-3 rounded-full border border-black/10"
        aria-label={t($locale, 'Maximize or restore window')}
        onclick={() => runWindowAction('maximize')}
      ></button>
    </div>
  {/if}

  <button
    class="titlebar-brand flex min-w-0 cursor-grab items-center gap-2.5 border-0 bg-transparent px-3 text-left text-foreground active:cursor-grabbing focus-visible:outline-2 focus-visible:outline-offset-[-3px] focus-visible:outline-ring"
    aria-label={t($locale, 'Drag window')}
    title={t($locale, 'Drag window')}
    onpointerdown={(event) => void startDragging(event)}
  >
    <img
      class="size-7 shrink-0 rounded-md object-contain"
      src={appIcon}
      alt=""
      draggable="false"
    />
    <strong class="text-xs font-semibold">RambleDesk</strong>
    <span class="h-4 w-px bg-border"></span>
    <span class="max-w-56 truncate text-[10px] text-muted-foreground">{sourceLabel}</span>
  </button>

  <button
    class="titlebar-drag min-w-6 flex-1 cursor-grab border-0 bg-transparent active:cursor-grabbing focus-visible:outline-2 focus-visible:outline-offset-[-3px] focus-visible:outline-ring"
    aria-label={t($locale, 'Drag window')}
    title={t($locale, 'Drag window')}
    onpointerdown={(event) => void startDragging(event)}
  ></button>

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
  </div>

  {#if !isMac}
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
</header>

<style>
  .traffic.close {
    background: #ff5f57;
  }

  .traffic.minimize {
    background: #febc2e;
  }

  .traffic.maximize {
    background: #28c840;
  }
</style>

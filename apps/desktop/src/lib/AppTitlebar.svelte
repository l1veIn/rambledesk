<script lang="ts">
  import { CircleAlert, Copy, Inbox, LoaderCircle, Mic, Minus, Pause, Square, X } from '@lucide/svelte'
  import { onMount, type Snippet } from 'svelte'

  import appIcon from '../assets/rambledesk-app-icon.webp'
  import { Badge } from '$lib/components/ui/badge'
  import { createUnavailableWorkbenchCapabilities } from '$lib/capabilities/unavailableCapabilities'
  import type {
    CapabilitySlot,
    WindowCapability,
  } from '$lib/capabilities/workbenchCapabilities'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import { titlebarPointerIntent } from '$lib/titlebarInteractions'
  import type { RamblePhase } from '$lib/workbench/types'

  const unavailableCapabilities = createUnavailableWorkbenchCapabilities()

  export let sidebarCollapsed = false
  export let workspaceTabs: Snippet
  export let pendingCount = 0
  export let ramblePhase: RamblePhase = 'idle'
  export let rambleRequestTitle = ''
  export let windowControls: CapabilitySlot<WindowCapability> = unavailableCapabilities.windowControls
  export let onWindowError: (message: string) => void = () => {}

  $: windowControlsAvailable = windowControls.status.availability !== 'unavailable'
  $: isMac = windowControls.implementation.platform() === 'macOS'
  $: rambleStatusLabel = ramblePhase === 'active'
    ? t($locale, 'Recording')
    : ramblePhase === 'starting'
      ? t($locale, 'Starting…')
      : ramblePhase === 'stopping'
        ? t($locale, 'Pausing…')
        : ramblePhase === 'error'
          ? t($locale, 'Ramble error')
          : t($locale, 'Ramble paused')
  $: rambleStatusTitle = [rambleStatusLabel, rambleRequestTitle].filter(Boolean).join(' · ')
  $: pendingLabel = `${pendingCount} ${t($locale, 'pending')}`
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

  function isInteractiveTitlebarTarget(target: EventTarget | null) {
    if (!(target instanceof Element)) return false
    const button = target.closest('button')
    if (button?.matches('.titlebar-brand')) return false
    return Boolean(button || target.closest('[role="tab"], [data-workspace-tab-item]'))
  }

  async function handleTitlebarPointerDown(event: PointerEvent) {
    if (!windowControlsAvailable) return
    const intent = titlebarPointerIntent({
      button: event.button,
      clickCount: event.detail,
      interactive: isInteractiveTitlebarTarget(event.target),
    })
    if (intent === 'ignore') return
    try {
      if (intent === 'toggle-maximize') {
        await windowControls.implementation.toggleMaximize()
        if (!isMac) maximized = await windowControls.implementation.isMaximized()
      } else {
        await windowControls.implementation.startDragging()
      }
    } catch (cause) {
      onWindowError(cause instanceof Error ? cause.message : String(cause))
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions (the handler delegates native titlebar dragging while preserving interactive descendants) -->
<header
  class={[
    'app-titlebar relative z-30 flex h-10 select-none items-stretch overflow-hidden rounded-t-[15px]',
  ]}
  data-titlebar-event-boundary
  onpointerdown={(event) => void handleTitlebarPointerDown(event)}
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

  <div class="flex min-w-0 flex-1 items-stretch">
    <div class="min-w-0 flex-1">
      {@render workspaceTabs()}
    </div>

    {#if ramblePhase !== 'idle' || pendingCount > 0}
      <div class="flex shrink-0 items-center gap-1 px-1.5">
        {#if ramblePhase !== 'idle'}
          <span
            class={[
              'grid size-6 shrink-0 place-items-center rounded-full',
              ramblePhase === 'active' || ramblePhase === 'error'
                ? 'bg-destructive/10 text-destructive'
                : 'bg-warning/10 text-warning-foreground dark:text-warning',
            ]}
            title={rambleStatusTitle}
            role="status"
          >
            {#if ramblePhase === 'starting' || ramblePhase === 'stopping'}
              <LoaderCircle class="size-3.5 motion-safe:animate-spin" aria-hidden="true" />
            {:else if ramblePhase === 'active'}
              <Mic class="size-3.5 motion-safe:animate-pulse" aria-hidden="true" />
            {:else if ramblePhase === 'error'}
              <CircleAlert class="size-3.5" aria-hidden="true" />
            {:else}
              <Pause class="size-3.5" aria-hidden="true" />
            {/if}
            <span class="sr-only">{rambleStatusTitle}</span>
          </span>
        {/if}
        {#if pendingCount > 0}
          <Badge
            variant="secondary"
            class="h-6 gap-1 bg-warning/10 px-1.5 text-[10px] tabular-nums text-warning-foreground dark:text-warning"
            title={pendingLabel}
          >
            <Inbox class="size-3" aria-hidden="true" />
            <span aria-hidden="true">{pendingCount > 99 ? '99+' : pendingCount}</span>
            <span class="sr-only">{pendingLabel}</span>
          </Badge>
        {/if}
      </div>
    {/if}

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
  .app-titlebar {
    background: var(--titlebar-background);
    /* Keep the divider as the only bottom edge, including after CSS-only hot updates. */
    border-bottom: 0;
  }

  .app-titlebar::after {
    position: absolute;
    right: 0;
    bottom: 0;
    left: 0;
    z-index: 1;
    height: 0;
    content: '';
    border-bottom: 1px solid var(--border);
    pointer-events: none;
    /* The active tab (z-10), including its reverse corners, covers this divider. */
  }

  .traffic.close {
    background: #ff5f57;
  }

  .traffic.minimize {
    background: #febc2e;
  }
</style>

<script lang="ts">
  import type { Snippet } from 'svelte'
  import { onMount, tick } from 'svelte'
  import { List } from '@lucide/svelte'
  import NavigationResizeHandle from '$lib/components/navigation/NavigationResizeHandle.svelte'
  import { COLLAPSED_RAIL_WIDTH, fitNavigationWidths, RAIL_LIMITS } from '$lib/components/navigation/railResize'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import {
    initialHostRailWidth,
    initialRequestRailWidth,
    saveHostRailWidth,
    saveRequestRailWidth,
  } from '$lib/uiPreferences'
  import { PHONE_QUERY, shellModeFor, TABLET_QUERY, type ShellMode } from './shellMode'

  // Collapse is owned by App.svelte: on phones it means "drawer closed", otherwise the rail
  // preference. The titlebar and both rails read the same effective value.
  export let hostCollapsed = false
  export let requestCollapsed = false
  export let onHostCollapsedChange: (collapsed: boolean) => void = () => {}
  export let onRequestCollapsedChange: (collapsed: boolean) => void = () => {}
  export let startupFailed = false
  export let requestPaneVisible = true
  /** Out: viewport class that selects the column or drawer presentation. */
  export let mode: ShellMode = 'desktop'
  /** Out: fitted host rail width, published as the `--workbench-sidebar-width` custom property. */
  export let hostDisplayWidth = 0
  /** Out: true while either rail is being dragged. */
  export let resizing = false

  export let startupRecovery: Snippet | undefined = undefined
  export let hostRail: Snippet
  export let requestPane: Snippet
  export let workspacePane: Snippet

  let hostRailWidth = initialHostRailWidth()
  let requestRailWidth = initialRequestRailWidth()
  let navigationWidth = 1320
  let resizingHostRail = false
  let resizingRequestRail = false
  let hostPaneElement: HTMLDivElement
  let requestPaneElement: HTMLDivElement
  let focusedDrawer: 'host' | 'request' | null = null

  $: isPhone = mode === 'phone'
  $: showRequestPane = !startupFailed && requestPaneVisible
  $: navigationWidths = fitNavigationWidths({
    hostWidth: hostRailWidth,
    requestWidth: showRequestPane ? requestRailWidth : null,
    hostCollapsed,
    requestCollapsed,
    containerWidth: navigationWidth,
  })
  $: hostRailMaxWidth = Math.max(RAIL_LIMITS.host.minWidth, Math.min(RAIL_LIMITS.host.maxWidth, navigationWidth - navigationWidths.request - 640))
  $: requestRailMaxWidth = Math.max(RAIL_LIMITS.request.minWidth, Math.min(RAIL_LIMITS.request.maxWidth, navigationWidth - navigationWidths.host - 640))
  $: hostDisplayWidth = isPhone ? COLLAPSED_RAIL_WIDTH : navigationWidths.host
  $: resizing = !isPhone && (resizingHostRail || resizingRequestRail)
  $: hostDrawerOpen = isPhone && !hostCollapsed
  $: requestDrawerOpen = isPhone && showRequestPane && !requestCollapsed
  $: drawerOpen = hostDrawerOpen || requestDrawerOpen

  // The reopen button disappears when its drawer opens, so move focus into the drawer instead
  // of letting it fall back to the document body.
  $: {
    const next = isPhone ? (hostDrawerOpen ? 'host' : requestDrawerOpen ? 'request' : null) : null
    if (next !== focusedDrawer) {
      focusedDrawer = next
      if (next === 'host') void tick().then(() => hostPaneElement?.focus({ preventScroll: true }))
      else if (next === 'request') void tick().then(() => requestPaneElement?.focus({ preventScroll: true }))
    }
  }

  function tr(source: string) {
    return t($locale, source)
  }

  function closeDrawers() {
    if (hostDrawerOpen) onHostCollapsedChange(true)
    if (requestDrawerOpen) onRequestCollapsedChange(true)
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key !== 'Escape' || !drawerOpen) return
    event.stopPropagation()
    closeDrawers()
  }

  onMount(() => {
    const phone = window.matchMedia(PHONE_QUERY)
    const tablet = window.matchMedia(TABLET_QUERY)
    const update = () => {
      mode = shellModeFor(phone.matches, tablet.matches)
    }
    update()
    phone.addEventListener('change', update)
    tablet.addEventListener('change', update)
    return () => {
      phone.removeEventListener('change', update)
      tablet.removeEventListener('change', update)
    }
  })
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="relative flex min-h-0 min-w-0 flex-1" bind:clientWidth={navigationWidth}>
  {#if drawerOpen}
    <button
      type="button"
      class="shell-drawer-backdrop"
      aria-label={tr('Close navigation')}
      onclick={closeDrawers}
    ></button>
  {/if}

  <!-- svelte-ignore a11y_no_noninteractive_tabindex (Phone drawers move focus here when they open so it does not fall back to the document body.) -->
  <div
    id="host-navigation-pane"
    data-navigation-pane
    bind:this={hostPaneElement}
    tabindex={isPhone ? -1 : undefined}
    class={[
      'relative flex min-h-0 shrink-0 outline-none',
      isPhone ? 'shell-drawer' : 'transition-[width] duration-200 motion-reduce:transition-none',
      hostDrawerOpen ? 'shell-drawer-open' : '',
    ]}
    style={isPhone ? undefined : `width: ${navigationWidths.host}px`}
    inert={isPhone && !hostDrawerOpen}
  >
    {@render hostRail()}
    {#if !isPhone}
      <NavigationResizeHandle
        label={$locale === 'zh-CN' ? '调整侧边栏宽度' : 'Resize sidebar'}
        controls="host-navigation-pane"
        expandedWidth={hostRailWidth}
        displayWidth={navigationWidths.host}
        collapsed={hostCollapsed}
        minWidth={RAIL_LIMITS.host.minWidth}
        maxWidth={hostRailMaxWidth}
        onResize={(next) => { hostRailWidth = next.width; onHostCollapsedChange(next.collapsed) }}
        onCommit={() => saveHostRailWidth(hostRailWidth)}
        onDraggingChange={(active) => resizingHostRail = active}
      />
    {/if}
  </div>

  <div class="appearance-workspace flex min-h-0 min-w-0 flex-1" id="request-workspace-layout">
    {#if startupFailed}
      {#if startupRecovery}{@render startupRecovery()}{/if}
    {:else}
      {#if showRequestPane && !isPhone}{@render requestListPane()}{/if}

      <div class="min-h-0 min-w-0 flex-1" id="workspace-pane" inert={drawerOpen}>
        <div class="flex h-full min-h-0 min-w-0 flex-col">
          {@render workspacePane()}
        </div>
      </div>
    {/if}
  </div>

  {#if isPhone && showRequestPane}{@render requestListPane()}{/if}

  {#if isPhone && !drawerOpen && showRequestPane && requestCollapsed}
    <button
      type="button"
      class="shell-fab shell-fab-left"
      aria-label={tr('Open request list')}
      onclick={() => onRequestCollapsedChange(false)}
    >
      <List aria-hidden="true" />
    </button>
  {/if}
</div>

{#snippet requestListPane()}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex (Phone drawers move focus here when they open so it does not fall back to the document body.) -->
  <div
    class={[
      'relative shrink-0 outline-none',
      isPhone ? 'shell-drawer' : 'border-r transition-[width] duration-200 motion-reduce:transition-none',
      requestDrawerOpen ? 'shell-drawer-open' : '',
    ]}
    id="request-list-pane"
    data-navigation-pane
    bind:this={requestPaneElement}
    tabindex={isPhone ? -1 : undefined}
    style={isPhone ? undefined : `width: ${navigationWidths.request}px`}
    inert={isPhone && !requestDrawerOpen}
  >
    {@render requestPane()}
    {#if !isPhone}
      <NavigationResizeHandle
        label={$locale === 'zh-CN' ? '调整请求列宽度' : 'Resize request list'}
        controls="request-list-pane"
        expandedWidth={requestRailWidth}
        displayWidth={navigationWidths.request}
        collapsed={requestCollapsed}
        minWidth={RAIL_LIMITS.request.minWidth}
        maxWidth={requestRailMaxWidth}
        onResize={(next) => { requestRailWidth = next.width; onRequestCollapsedChange(next.collapsed) }}
        onCommit={() => saveRequestRailWidth(requestRailWidth)}
        onDraggingChange={(active) => resizingRequestRail = active}
      />
    {/if}
  </div>
{/snippet}

<style>
  .shell-drawer {
    position: absolute;
    inset: 0 auto 0 0;
    z-index: 40;
    width: min(85vw, 20rem);
    --workbench-sidebar-width: min(85vw, 20rem);
    transform: translateX(-100%);
    transition: transform 200ms ease;
  }

  .shell-drawer > :global(*) {
    height: 100%;
  }

  .shell-drawer-open {
    transform: translateX(0);
    box-shadow: 0 12px 32px rgb(0 0 0 / 0.28);
  }

  .shell-drawer-backdrop {
    position: absolute;
    inset: 0;
    z-index: 35;
    border: 0;
    padding: 0;
    background: rgb(0 0 0 / 0.35);
  }

  .shell-fab {
    position: absolute;
    bottom: 0.75rem;
    z-index: 30;
    display: grid;
    place-items: center;
    width: 2.75rem;
    height: 2.75rem;
    border: 1px solid var(--border);
    border-radius: 9999px;
    background: var(--background);
    color: var(--foreground);
    box-shadow: 0 6px 16px rgb(0 0 0 / 0.18);
  }

  .shell-fab-left {
    left: 0.75rem;
  }

  @media (prefers-reduced-motion: reduce) {
    .shell-drawer {
      transition: none;
    }
  }
</style>

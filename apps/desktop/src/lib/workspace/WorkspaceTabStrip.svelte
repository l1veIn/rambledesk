<script lang="ts">
  import { flip } from 'svelte/animate'
  import { tick } from 'svelte'
  import { X } from '@lucide/svelte'
  import { dndzone, type DndEvent } from 'svelte-dnd-action'

  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import {
    workspaceViewKey,
    type WorkspaceViewDescriptor,
  } from './viewDescriptors'
  import { tabStripLayout } from './tabStripLayout'
  import {
    requestWorkspaceTabActivation,
    workspaceTabId,
    workspaceTabKeyboardAction,
    workspaceTabNavigationTarget,
    workspaceTabPanelId,
    type WorkspaceTabNavigationIntent,
  } from './workspaceTabNavigation'

  type WorkspaceTabDndItem = {
    id: string
    view: WorkspaceViewDescriptor
  }

  const FLIP_DURATION_MS = 140
  const WORKSPACE_TAB_DND_TYPE = 'rambledesk-workspace-tab'
  /** Left padding (0.5rem) plus the drag surface (2.5rem) tabs must leave alone. */
  const STRIP_RESERVED_WIDTH = 48

  export let views: readonly WorkspaceViewDescriptor[] = []
  export let activeViewKey: string | null = null
  export let pendingViewKey: string | null = null
  export let disabled = false
  export let labelForView: (view: WorkspaceViewDescriptor) => string
  export let onActivate: (viewKey: string) => void = () => {}
  export let onClose: (viewKey: string) => Promise<void> | void = () => {}
  export let onReorder: (viewKeys: readonly string[]) => void = () => {}

  let dndItems: WorkspaceTabDndItem[] = []
  let dragging = false
  let viewKeys: string[] = []
  let focusedViewKey: string | null = null
  let tabButtons = new Map<string, HTMLElement>()
  let tabListElement: HTMLDivElement
  let scrolledViewKey: string | null = null
  let stripWidth = 0
  let listClientWidth = 0
  let listScrollWidth = 0
  let listScrollLeft = 0
  let previousTabCount = 0

  // One rule for every viewport: tabs share the strip evenly, shrink to a floor,
  // then the strip scrolls.
  $: layout = tabStripLayout(Math.max(0, stripWidth - STRIP_RESERVED_WIDTH), dndItems.length)
  $: canScrollLeft = listScrollLeft > 1
  $: canScrollRight = listScrollLeft + listClientWidth < listScrollWidth - 1
  // Tab set or width changed: re-measure so the fade affordances stay accurate.
  $: if (tabListElement && (viewKeys.join('\u0001') || layout.tabWidth || listClientWidth)) {
    void tick().then(measureList)
  }

  $: if (!dragging) {
    dndItems = views.map((view) => ({ id: workspaceViewKey(view), view }))
  }
  $: viewKeys = dndItems.map((item) => item.id)
  $: if (!focusedViewKey || !viewKeys.includes(focusedViewKey)) {
    focusedViewKey = activeViewKey && viewKeys.includes(activeViewKey) ? activeViewKey : viewKeys[0] ?? null
  }
  $: if (activeViewKey && viewKeys.includes(activeViewKey)) {
    focusedViewKey = activeViewKey
  }
  // A newly opened tab lands at the end of the queue: follow it so the human sees
  // where the tab went even when the strip already scrolls.
  $: if (tabListElement && viewKeys.length !== previousTabCount) {
    const grew = viewKeys.length > previousTabCount
    previousTabCount = viewKeys.length
    if (grew) void tick().then(scrollToEnd)
  }

  // The strip scrolls once tabs reach the minimum width, so keep the active tab visible.
  $: if (tabListElement && activeViewKey !== scrolledViewKey) {
    const viewKey = activeViewKey
    scrolledViewKey = viewKey
    // The tab's element only exists after this flush; scrolling before that is a no-op.
    if (viewKey) void tick().then(() => revealTab(viewKey))
  }

  function tr(source: string) {
    return t($locale, source)
  }

  function measureList() {
    if (!tabListElement) return
    listScrollWidth = tabListElement.scrollWidth
    listScrollLeft = tabListElement.scrollLeft
  }

  function scrollToEnd() {
    if (!tabListElement) return
    tabListElement.scrollLeft = tabListElement.scrollWidth
    measureList()
  }

  /** Scrolls the minimum amount that makes a tab fully visible. */
  function revealTab(viewKey: string) {
    if (!tabListElement) return
    const index = viewKeys.indexOf(viewKey)
    if (index < 0) return
    const start = index * layout.tabWidth
    const end = start + layout.tabWidth
    const visibleStart = tabListElement.scrollLeft
    const visibleEnd = visibleStart + tabListElement.clientWidth
    if (start < visibleStart) tabListElement.scrollLeft = start
    else if (end > visibleEnd) tabListElement.scrollLeft = end - tabListElement.clientWidth
    measureList()
  }

  /** Desktop wheels scroll vertically: translate it while the strip overflows. */
  function handleWheel(event: WheelEvent) {
    if (!tabListElement || !layout.overflowing) return
    if (Math.abs(event.deltaY) <= Math.abs(event.deltaX)) return
    const before = tabListElement.scrollLeft
    tabListElement.scrollLeft = before + event.deltaY
    if (tabListElement.scrollLeft !== before) event.preventDefault()
  }

  function registerTab(node: HTMLElement, viewKey: string) {
    tabButtons.set(viewKey, node)
    return {
      destroy() {
        if (tabButtons.get(viewKey) === node) tabButtons.delete(viewKey)
      },
    }
  }

  function registerTabKeyboard(node: HTMLElement, initialViewKey: string) {
    let viewKey = initialViewKey
    const listener = (event: KeyboardEvent) => handleKeydown(event, viewKey)
    node.addEventListener('keydown', listener)
    return {
      update(nextViewKey: string) {
        viewKey = nextViewKey
      },
      destroy() {
        node.removeEventListener('keydown', listener)
      },
    }
  }

  async function focusTab(viewKey: string) {
    await tick()
    tabButtons.get(viewKey)?.focus()
  }

  function moveFocus(intent: WorkspaceTabNavigationIntent) {
    const nextViewKey = workspaceTabNavigationTarget(viewKeys, focusedViewKey, intent)
    if (!nextViewKey) return
    focusedViewKey = nextViewKey
    void focusTab(nextViewKey)
  }

  function handleKeydown(event: KeyboardEvent, viewKey: string) {
    const action = workspaceTabKeyboardAction(event.key)
    if (!action) return
    event.preventDefault()
    event.stopPropagation()
    if (action.type === 'move') moveFocus(action.intent)
    else if (action.type === 'activate') activateTab(viewKey)
    else void closeAndFocus(viewKey)
  }

  function activateTab(viewKey: string) {
    focusedViewKey = viewKey
    const requested = requestWorkspaceTabActivation(
      viewKey,
      disabled || pendingViewKey !== null,
      onActivate,
    )
    if (requested) void focusTab(viewKey)
  }

  async function closeAndFocus(viewKey: string) {
    if (disabled || pendingViewKey !== null) return
    await onClose(viewKey)
    await tick()
    const nextViewKey =
      activeViewKey && viewKeys.includes(activeViewKey) ? activeViewKey : viewKeys[0] ?? null
    if (!nextViewKey) return
    focusedViewKey = nextViewKey
    await focusTab(nextViewKey)
  }

  function handleMiddleClick(event: MouseEvent, viewKey: string) {
    if (event.button !== 1) return
    event.preventDefault()
    void closeAndFocus(viewKey)
  }

  function handleDndConsider(event: CustomEvent<DndEvent<WorkspaceTabDndItem>>) {
    dragging = true
    dndItems = event.detail.items
  }

  function handleDndFinalize(event: CustomEvent<DndEvent<WorkspaceTabDndItem>>) {
    dndItems = event.detail.items
    onReorder(dndItems.map((item) => item.id))
    dragging = false
  }
</script>

<div class="flex h-full min-w-0 items-stretch overflow-hidden pl-2 pt-1.5" bind:clientWidth={stripWidth}>
  {#if dndItems.length > 0}
    <div
      use:dndzone={{
        items: dndItems,
        type: WORKSPACE_TAB_DND_TYPE,
        flipDurationMs: FLIP_DURATION_MS,
        dragDisabled: disabled || pendingViewKey !== null,
        morphDisabled: true,
        dropFromOthersDisabled: true,
        zoneTabIndex: -1,
        zoneItemTabIndex: -1,
        dropTargetStyle: {},
        autoAriaDisabled: true,
        delayTouchStart: 500,
      }}
      class="workspace-tab-list flex h-full min-w-0 flex-[0_1_auto] items-stretch overflow-x-auto overflow-y-hidden"
      style:--tab-fade-start={canScrollLeft ? '1.25rem' : '0px'}
      style:--tab-fade-end={canScrollRight ? '1.25rem' : '0px'}
      bind:this={tabListElement}
      bind:clientWidth={listClientWidth}
      onscroll={measureList}
      onwheel={handleWheel}
      role="tablist"
      aria-label={tr('Workspace tabs')}
      aria-orientation="horizontal"
      aria-busy={pendingViewKey !== null}
      data-workspace-tab-list
      onconsider={handleDndConsider}
      onfinalize={handleDndFinalize}
    >
      {#each dndItems as item (item.id)}
        {@const viewKey = item.id}
        {@const label = labelForView(item.view)}
        <div
          animate:flip={{ duration: FLIP_DURATION_MS }}
          class="workspace-tab-item relative shrink-0 cursor-grab active:cursor-grabbing"
          style:width={`${layout.tabWidth}px`}
          class:z-10={activeViewKey === viewKey}
          data-workspace-tab-item
          data-workspace-view-key={viewKey}
          data-active={activeViewKey === viewKey ? 'true' : 'false'}
        >
          <span class="workspace-tab-seat" aria-hidden="true"></span>
          <!-- svelte-ignore a11y_click_events_have_key_events (the native keyboard action above preserves Tab semantics before DnD sees the event) -->
          <div
            use:registerTab={viewKey}
            use:registerTabKeyboard={viewKey}
            role="tab"
            id={workspaceTabId(viewKey)}
            aria-controls={workspaceTabPanelId(viewKey)}
            aria-selected={activeViewKey === viewKey}
            aria-disabled={disabled || pendingViewKey !== null}
            data-workspace-tab-trigger
            data-workspace-view-key={viewKey}
            tabindex={focusedViewKey === viewKey ? 0 : -1}
            class="workspace-tab-content group/tab relative flex h-full w-full min-w-0 items-center overflow-hidden rounded-t-lg px-2 pb-1.5 text-left text-xs outline-none transition-colors focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50"
            class:workspace-tab-active={activeViewKey === viewKey}
            class:workspace-tab-inactive={activeViewKey !== viewKey}
            title={label}
            onclick={() => activateTab(viewKey)}
            onauxclick={(event) => handleMiddleClick(event, viewKey)}
          >
            <span class="workspace-tab-label pointer-events-none min-w-0 flex-1 overflow-hidden whitespace-nowrap">
              {label}
            </span>
          </div>
          <button
            type="button"
            class="workspace-tab-close absolute bottom-1.5 right-1 top-0 my-auto grid size-6 place-items-center rounded-md bg-background text-muted-foreground outline-none transition-opacity hover:bg-foreground/10 hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-40 pointer-coarse:size-8"
            class:workspace-tab-close-active={activeViewKey === viewKey}
            aria-label={`${tr('Close workspace tab')}: ${label}`}
            title={tr('Close workspace tab')}
            tabindex={activeViewKey === viewKey ? 0 : -1}
            disabled={disabled || pendingViewKey !== null}
            onclick={(event) => {
              event.stopPropagation()
              void closeAndFocus(viewKey)
            }}
          >
            <X class="size-3" aria-hidden="true" />
          </button>
        </div>
      {/each}
    </div>
  {/if}

  <div
    class="titlebar-drag h-full min-w-10 flex-1 cursor-grab active:cursor-grabbing"
    data-titlebar-drag-surface
    title={tr('Drag window')}
  ></div>
</div>

<style>
  .workspace-tab-item::before {
    position: absolute;
    top: calc(50% - 3px);
    left: 0;
    width: 1px;
    height: 1rem;
    content: '';
    background: var(--border);
    opacity: 1;
    pointer-events: none;
    transform: translateY(-50%);
    transition: opacity 150ms ease;
  }

  .workspace-tab-item:first-child::before,
  .workspace-tab-item[data-active='true']::before,
  .workspace-tab-item:hover::before,
  .workspace-tab-item[data-active='true'] + .workspace-tab-item::before,
  .workspace-tab-item:hover + .workspace-tab-item::before {
    opacity: 0;
  }

  .workspace-tab-content {
    isolation: isolate;
    cursor: inherit;
    color: var(--muted-foreground);
  }

  .workspace-tab-active {
    padding-right: 2rem;
    color: var(--foreground);
    background: var(--background);
  }

  .workspace-tab-inactive::before {
    position: absolute;
    inset: 0.125rem 0.125rem 0.5rem;
    z-index: -1;
    content: '';
    background: color-mix(in oklab, var(--background) 60%, transparent);
    border-radius: 0.5rem;
    opacity: 0;
    pointer-events: none;
    transition: opacity 150ms ease;
  }

  .workspace-tab-inactive:hover::before {
    opacity: 1;
  }

  .workspace-tab-label {
    padding-right: 0.75rem;
    -webkit-mask-image: linear-gradient(to right, #000 calc(100% - 0.75rem), transparent);
    mask-image: linear-gradient(to right, #000 calc(100% - 0.75rem), transparent);
  }

  .workspace-tab-item:not([data-active='true']):hover .workspace-tab-label {
    -webkit-mask-image: linear-gradient(
      to right,
      #000 calc(100% - 1.75rem),
      transparent calc(100% - 1rem)
    );
    mask-image: linear-gradient(
      to right,
      #000 calc(100% - 1.75rem),
      transparent calc(100% - 1rem)
    );
  }

  .workspace-tab-close {
    opacity: 0;
    pointer-events: none;
  }

  .workspace-tab-item:hover .workspace-tab-close,
  .workspace-tab-close-active,
  .workspace-tab-close:focus-visible {
    opacity: 1;
    pointer-events: auto;
  }

  /* Touch has no hover: every tab keeps its close affordance reachable, and the label
     reserves room for it so the chip never sits on top of the title. */
  @media (pointer: coarse) {
    .workspace-tab-close {
      opacity: 1;
      pointer-events: auto;
    }

    .workspace-tab-content {
      padding-right: 2.5rem;
    }
  }

  .workspace-tab-list {
    scrollbar-width: none;
    overscroll-behavior-x: contain;
    -webkit-mask-image: linear-gradient(
      to right,
      transparent 0,
      #000 var(--tab-fade-start, 0px),
      #000 calc(100% - var(--tab-fade-end, 0px)),
      transparent 100%
    );
    mask-image: linear-gradient(
      to right,
      transparent 0,
      #000 var(--tab-fade-start, 0px),
      #000 calc(100% - var(--tab-fade-end, 0px)),
      transparent 100%
    );
  }

  .workspace-tab-list::-webkit-scrollbar {
    display: none;
  }

  .workspace-tab-seat {
    position: absolute;
    right: 0;
    bottom: 0;
    left: 0;
    height: 0.5rem;
    opacity: 0;
    pointer-events: none;
    transition: opacity 150ms ease;
  }

  .workspace-tab-item[data-active='true'] .workspace-tab-seat {
    opacity: 1;
  }

  .workspace-tab-seat::before,
  .workspace-tab-seat::after {
    position: absolute;
    bottom: 0;
    width: 0.5rem;
    height: 0.5rem;
    content: '';
    background: var(--background);
  }

  .workspace-tab-seat::before {
    left: -0.5rem;
    -webkit-mask-image: radial-gradient(circle at top left, transparent 0.4375rem, #000 0.5rem);
    mask-image: radial-gradient(circle at top left, transparent 0.4375rem, #000 0.5rem);
  }

  .workspace-tab-seat::after {
    right: -0.5rem;
    -webkit-mask-image: radial-gradient(circle at top right, transparent 0.4375rem, #000 0.5rem);
    mask-image: radial-gradient(circle at top right, transparent 0.4375rem, #000 0.5rem);
  }

  @media (prefers-reduced-motion: reduce) {
    .workspace-tab-item::before,
    .workspace-tab-inactive::before,
    .workspace-tab-close,
    .workspace-tab-seat {
      transition: none;
    }
  }
</style>

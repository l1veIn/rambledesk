<script lang="ts">
  import { tick } from 'svelte'
  import { X } from '@lucide/svelte'

  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import {
    workspaceViewKey,
    type WorkspaceViewDescriptor,
  } from './viewDescriptors'
  import {
    requestWorkspaceTabActivation,
    workspaceTabId,
    workspaceTabKeyboardAction,
    workspaceTabNavigationTarget,
    workspaceTabPanelId,
    type WorkspaceTabNavigationIntent,
  } from './workspaceTabNavigation'

  export let views: readonly WorkspaceViewDescriptor[] = []
  export let activeViewKey: string | null = null
  export let pendingViewKey: string | null = null
  export let disabled = false
  export let labelForView: (view: WorkspaceViewDescriptor) => string
  export let onActivate: (viewKey: string) => void = () => {}
  export let onClose: (viewKey: string) => Promise<void> | void = () => {}

  let viewKeys: string[] = []
  let focusedViewKey: string | null = null
  let tabButtons = new Map<string, HTMLButtonElement>()
  let lastScrolledActiveViewKey: string | null = null

  $: viewKeys = views.map(workspaceViewKey)
  $: if (!focusedViewKey || !viewKeys.includes(focusedViewKey)) {
    focusedViewKey = activeViewKey && viewKeys.includes(activeViewKey) ? activeViewKey : viewKeys[0] ?? null
  }
  $: if (activeViewKey !== lastScrolledActiveViewKey) {
    lastScrolledActiveViewKey = activeViewKey
    if (activeViewKey) {
      focusedViewKey = activeViewKey
      void revealTab(activeViewKey, false)
    }
  }

  function tr(source: string) {
    return t($locale, source)
  }

  function registerTab(node: HTMLButtonElement, viewKey: string) {
    tabButtons.set(viewKey, node)
    return {
      destroy() {
        if (tabButtons.get(viewKey) === node) tabButtons.delete(viewKey)
      },
    }
  }

  async function revealTab(viewKey: string, focus: boolean) {
    await tick()
    const tab = tabButtons.get(viewKey)
    if (!tab) return
    tab.scrollIntoView({ block: 'nearest', inline: 'nearest' })
    if (focus) tab.focus()
  }

  function moveFocus(intent: WorkspaceTabNavigationIntent) {
    const nextViewKey = workspaceTabNavigationTarget(viewKeys, focusedViewKey, intent)
    if (!nextViewKey) return
    focusedViewKey = nextViewKey
    void revealTab(nextViewKey, true)
  }

  function handleKeydown(event: KeyboardEvent, viewKey: string) {
    const action = workspaceTabKeyboardAction(event.key)
    if (!action) return
    event.preventDefault()
    if (action.type === 'move') moveFocus(action.intent)
    else if (action.type === 'activate') activateTab(viewKey)
    else void closeAndFocus(viewKey)
  }

  function activateTab(viewKey: string) {
    focusedViewKey = viewKey
    requestWorkspaceTabActivation(viewKey, disabled || pendingViewKey !== null, onActivate)
  }

  async function closeAndFocus(viewKey: string) {
    await onClose(viewKey)
    await tick()
    const nextViewKey =
      activeViewKey && viewKeys.includes(activeViewKey) ? activeViewKey : viewKeys[0] ?? null
    if (!nextViewKey) return
    focusedViewKey = nextViewKey
    await revealTab(nextViewKey, true)
  }
</script>

{#if views.length > 0}
  <div class="shrink-0 border-b bg-muted/25 px-2 pt-1.5">
    <div
      class="overflow-x-auto overscroll-x-contain [scrollbar-width:thin]"
      role="tablist"
      aria-label={tr('Workspace tabs')}
      aria-orientation="horizontal"
      aria-busy={pendingViewKey !== null}
    >
      <div class="flex min-w-max items-end gap-1">
        {#each views as view (workspaceViewKey(view))}
          {@const viewKey = workspaceViewKey(view)}
          {@const label = labelForView(view)}
          <div
            role="presentation"
            class={[
              'flex max-w-56 shrink-0 items-center rounded-t-md border border-b-0 transition-colors',
              activeViewKey === viewKey
                ? 'bg-background text-foreground'
                : 'border-transparent text-muted-foreground hover:bg-background/60 hover:text-foreground',
            ]}
          >
            <button
              use:registerTab={viewKey}
              type="button"
              role="tab"
              id={workspaceTabId(viewKey)}
              data-workspace-view-key={viewKey}
              data-active={activeViewKey === viewKey ? 'true' : 'false'}
              aria-controls={workspaceTabPanelId(viewKey)}
              aria-selected={activeViewKey === viewKey}
              tabindex={focusedViewKey === viewKey ? 0 : -1}
              class={[
                'min-w-0 flex-1 truncate rounded-tl-md px-3 py-1.5 text-left text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring/50',
                activeViewKey === viewKey ? 'font-medium' : undefined,
              ]}
              title={label}
              disabled={disabled || pendingViewKey !== null}
              onclick={() => activateTab(viewKey)}
              onkeydown={(event) => handleKeydown(event, viewKey)}
            >
              {label}
            </button>
            <button
              type="button"
              class="mr-1 grid size-5 shrink-0 place-items-center rounded-sm text-muted-foreground outline-none hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-40"
              aria-label={`${tr('Close workspace tab')}: ${label}`}
              title={tr('Close workspace tab')}
              tabindex={activeViewKey === viewKey ? 0 : -1}
              disabled={disabled || pendingViewKey !== null}
              onclick={() => void closeAndFocus(viewKey)}
            >
              <X class="size-3" aria-hidden="true" />
            </button>
          </div>
        {/each}
      </div>
    </div>
  </div>
{/if}

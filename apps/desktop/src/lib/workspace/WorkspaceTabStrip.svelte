<script lang="ts">
  import { tick } from 'svelte'

  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import {
    workspaceViewKey,
    type WorkspaceViewDescriptor,
  } from './viewDescriptors'
  import {
    workspaceTabNavigationTarget,
    type WorkspaceTabNavigationIntent,
  } from './workspaceTabNavigation'

  export let views: readonly WorkspaceViewDescriptor[] = []
  export let activeViewKey: string | null = null
  export let labelForView: (view: WorkspaceViewDescriptor) => string
  export let onActivate: (viewKey: string) => void = () => {}

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
    if (activeViewKey) void revealTab(activeViewKey, false)
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
    if (event.key === 'ArrowLeft') {
      event.preventDefault()
      moveFocus('previous')
    } else if (event.key === 'ArrowRight') {
      event.preventDefault()
      moveFocus('next')
    } else if (event.key === 'Home') {
      event.preventDefault()
      moveFocus('first')
    } else if (event.key === 'End') {
      event.preventDefault()
      moveFocus('last')
    } else if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      onActivate(viewKey)
    }
  }
</script>

{#if views.length > 0}
  <div class="shrink-0 border-b bg-muted/25 px-2 pt-1.5">
    <div
      class="overflow-x-auto overscroll-x-contain [scrollbar-width:thin]"
      role="tablist"
      aria-label={tr('Workspace tabs')}
      aria-orientation="horizontal"
    >
      <div class="flex min-w-max items-end gap-1">
        {#each views as view (workspaceViewKey(view))}
          {@const viewKey = workspaceViewKey(view)}
          <button
            use:registerTab={viewKey}
            type="button"
            role="tab"
            aria-selected={activeViewKey === viewKey}
            tabindex={focusedViewKey === viewKey ? 0 : -1}
            class={[
              'max-w-52 shrink-0 truncate rounded-t-md border border-b-0 px-3 py-1.5 text-left text-xs outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring/50',
              activeViewKey === viewKey
                ? 'bg-background font-medium text-foreground'
                : 'border-transparent text-muted-foreground hover:bg-background/60 hover:text-foreground',
            ]}
            title={labelForView(view)}
            onclick={() => onActivate(viewKey)}
            onfocus={() => (focusedViewKey = viewKey)}
            onkeydown={(event) => handleKeydown(event, viewKey)}
          >
            {labelForView(view)}
          </button>
        {/each}
      </div>
    </div>
  </div>
{/if}

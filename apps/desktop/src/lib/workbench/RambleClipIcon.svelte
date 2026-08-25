<script lang="ts">
  import { FileText, StickyNote } from '@lucide/svelte'
  import { onMount } from 'svelte'

  import { Button } from '$lib/components/ui/button'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import { clipFlyTransform, nextSavedTranscript, type ClipFlyFrom } from './briefNotes'

  export let index = 1
  export let text = ''
  export let kind: 'clip' | 'note' = 'clip'
  export let align: 'left' | 'right' = 'left'
  export let placement: 'top' | 'bottom' = 'top'
  export let compact = false
  export let autoOpen = false
  export let flyFrom: ClipFlyFrom | null = null
  export let readOnly = false
  export let onSave: (text: string) => void = () => {}

  let open = false
  let draft = text
  let root: HTMLDivElement

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  $: if (!open) draft = text
  $: dirty = nextSavedTranscript(draft, text) !== null
  $: title =
    kind === 'note' ? tr('Note {index}', { index }) : tr('Ramble clip {index}', { index })
  $: hideLabel = kind === 'note' ? tr('Hide block note') : tr('Hide ramble clip')
  $: showLabel = kind === 'note' ? tr('Show recorded note') : tr('Show recorded speech')

  function toggleOpen() {
    if (open) {
      open = false
      draft = text
      return
    }
    draft = text
    open = true
  }

  function save() {
    const next = nextSavedTranscript(draft, text)
    if (!next) return
    onSave(next)
  }

  function onDraftKeydown(event: KeyboardEvent) {
    if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
      event.preventDefault()
      save()
    }
  }

  function focusWhenMounted(node: HTMLTextAreaElement) {
    queueMicrotask(() => {
      node.focus()
      const end = node.value.length
      node.setSelectionRange(end, end)
    })
  }

  onMount(() => {
    const onPointerDown = (event: PointerEvent) => {
      if (!root.contains(event.target as Node)) open = false
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') open = false
    }
    window.addEventListener('pointerdown', onPointerDown)
    window.addEventListener('keydown', onKeyDown)

    const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    let openTimer: number | undefined
    if (flyFrom && !reduceMotion) {
      const to = root.getBoundingClientRect()
      const motion = clipFlyTransform(flyFrom, to)
      root.style.transform = `translate(${motion.x}px, ${motion.y}px) scale(${motion.scale})`
      root.style.opacity = '0.72'
      const play = () => {
        root.style.transition =
          'transform 480ms cubic-bezier(0.18, 0.86, 0.22, 1), opacity 220ms ease-out'
        root.style.transform = 'none'
        root.style.opacity = '1'
      }
      requestAnimationFrame(() => requestAnimationFrame(play))
      if (autoOpen) {
        openTimer = window.setTimeout(() => {
          draft = text
          open = true
        }, 520)
      }
    } else if (autoOpen) {
      draft = text
      open = true
    }

    return () => {
      if (openTimer !== undefined) window.clearTimeout(openTimer)
      window.removeEventListener('pointerdown', onPointerDown)
      window.removeEventListener('keydown', onKeyDown)
    }
  })
</script>

<div bind:this={root} class="relative shrink-0 will-change-transform">
  <button
    type="button"
    class="grid {compact ? 'size-6' : 'size-8'} place-items-center rounded-md border bg-background text-muted-foreground transition-colors hover:border-primary/40 hover:text-foreground"
    aria-expanded={open}
    aria-label={open ? hideLabel : title}
    title={title}
    onclick={() => toggleOpen()}
  >
    {#if kind === 'note'}
      <StickyNote class={compact ? 'size-3.5' : 'size-4'} />
    {:else}
      <FileText class={compact ? 'size-3.5' : 'size-4'} />
    {/if}
    <span class="sr-only">{showLabel}</span>
  </button>
  {#if open}
    <div
      class="absolute z-50 w-[min(22rem,calc(100vw-4rem))] rounded-md border bg-popover p-3 text-xs leading-5 text-popover-foreground shadow-lg {placement === 'bottom'
        ? 'top-full mt-2'
        : 'bottom-full mb-2'} {align === 'right' ? 'right-0' : 'left-0'}"
      role="dialog"
      tabindex="-1"
      aria-label={title}
      onpointerdown={(event) => event.stopPropagation()}
    >
      <strong class="mb-1 block text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
        {title}
      </strong>
      <textarea
        class="mt-1 max-h-48 min-h-24 w-full resize-y rounded-md border bg-background px-2 py-1.5 text-xs leading-5 text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring"
        value={draft}
        readonly={readOnly}
        aria-label={title}
        use:focusWhenMounted
        oninput={(event) => (draft = (event.currentTarget as HTMLTextAreaElement).value)}
        onkeydown={onDraftKeydown}
      ></textarea>
      {#if !readOnly}
        <div class="mt-2 flex justify-end gap-2">
          <Button size="xs" disabled={!dirty} onclick={save}>{tr('Save')}</Button>
        </div>
      {/if}
    </div>
  {/if}
</div>

<script lang="ts">
  import { FileText } from '@lucide/svelte'
  import { onMount } from 'svelte'

  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import { clipFlyTransform, type ClipFlyFrom } from './briefNotes'

  export let index = 1
  export let text = ''
  export let flyFrom: ClipFlyFrom | null = null

  let open = false
  let root: HTMLDivElement

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
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
    }

    return () => {
      window.removeEventListener('pointerdown', onPointerDown)
      window.removeEventListener('keydown', onKeyDown)
    }
  })
</script>

<div bind:this={root} class="relative shrink-0 will-change-transform">
  <button
    type="button"
    class="grid size-8 place-items-center rounded-md border bg-background text-muted-foreground transition-colors hover:border-primary/40 hover:text-foreground"
    aria-expanded={open}
    aria-label={open ? tr('Hide ramble clip') : tr('Ramble clip {index}', { index })}
    title={tr('Ramble clip {index}', { index })}
    onclick={() => (open = !open)}
  >
    <FileText class="size-4" />
    <span class="sr-only">{tr('Show recorded speech')}</span>
  </button>
  {#if open}
    <div
      class="absolute bottom-full left-0 z-50 mb-2 w-[min(22rem,calc(100vw-4rem))] rounded-md border bg-popover p-3 text-xs leading-5 text-popover-foreground shadow-lg"
      role="tooltip"
    >
      <strong class="mb-1 block text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
        {tr('Ramble clip {index}', { index })}
      </strong>
      <p class="m-0 max-h-48 overflow-y-auto whitespace-pre-wrap">{text}</p>
    </div>
  {/if}
</div>

<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { ArrowLeft, ArrowRight, Check, X } from '@lucide/svelte'
  import { placeSpotlightCard, visibleSpotlightRect, type SpotlightLabels, type SpotlightRect, type SpotlightStep } from './spotlightGeometry'

  export let steps: readonly SpotlightStep[]
  export let scope: HTMLElement
  export let portrait: string
  export let labels: SpotlightLabels
  export let onDismiss: (reason: 'skip' | 'complete') => void

  const id = `workbench-tour-${crypto.randomUUID()}`
  let dialog: HTMLDialogElement
  let card: HTMLDivElement
  let nextButton: HTMLButtonElement
  let index = 0
  let mounted = false
  let dismissed = false
  let frame = 0
  let viewport = { width: 1024, height: 768 }
  let targetRect: SpotlightRect | null = null
  let cardSize = { width: 380, height: 270 }
  let resizeObserver: ResizeObserver | undefined
  let observedTarget: HTMLElement | null = null
  const originalScroll = new Map<HTMLElement, { top: number; left: number }>()

  $: step = steps[index]
  $: position = placeSpotlightCard(targetRect, cardSize, viewport)
  $: cutout = targetRect
    ? `M${targetRect.left},${targetRect.top}h${targetRect.width}v${targetRect.height}h-${targetRect.width}Z`
    : ''
  $: if (mounted && step) revealStep(step)

  function findTarget(step: SpotlightStep | undefined): HTMLElement | null {
    if (!step || !scope?.isConnected) return null
    try { return scope.querySelector<HTMLElement>(step.target) }
    catch { return null }
  }

  function measure() {
    frame = 0
    if (!mounted || dismissed) return
    viewport = { width: window.innerWidth, height: window.innerHeight }
    const target = findTarget(step)
    if (target !== observedTarget) {
      if (observedTarget) resizeObserver?.unobserve(observedTarget)
      observedTarget = target
      if (target) resizeObserver?.observe(target)
    }
    targetRect = target ? visibleSpotlightRect(target, viewport) : null
    const bounds = card?.getBoundingClientRect()
    if (bounds?.width && bounds.height && (bounds.width !== cardSize.width || bounds.height !== cardSize.height)) {
      cardSize = { width: bounds.width, height: bounds.height }
    }
  }

  function scheduleMeasure() {
    if (mounted && !dismissed && !frame) frame = requestAnimationFrame(measure)
  }

  function revealStep(value: SpotlightStep | undefined) {
    if (!mounted || dismissed) return
    // Moving the viewport is the only effect of a step; draft actions stay untouched.
    const target = findTarget(value)
    for (let ancestor = target?.parentElement; ancestor; ancestor = ancestor.parentElement) {
      if (!originalScroll.has(ancestor)) originalScroll.set(ancestor, { top: ancestor.scrollTop, left: ancestor.scrollLeft })
    }
    target?.scrollIntoView?.({ block: 'nearest', inline: 'nearest', behavior: 'instant' })
    scheduleMeasure()
  }

  function restoreScroll() {
    for (const [element, position] of originalScroll) {
      if (!element.isConnected) continue
      element.scrollTop = position.top
      element.scrollLeft = position.left
    }
    originalScroll.clear()
  }

  function dismiss(reason: 'skip' | 'complete') {
    if (dismissed) return
    dismissed = true
    closeDialog()
    restoreScroll()
    onDismiss(reason)
  }

  function closeDialog() {
    if (typeof dialog.close === 'function') dialog.close()
    else dialog.removeAttribute('open')
  }

  async function move(delta: number) {
    index = Math.max(0, Math.min(steps.length - 1, index + delta))
    await tick()
    nextButton?.focus({ preventScroll: true })
  }

  function keydown(event: KeyboardEvent) {
    if (event.key !== 'Tab') return
    const buttons = Array.from(dialog.querySelectorAll<HTMLButtonElement>('button:not(:disabled)'))
    const first = buttons[0]
    const last = buttons.at(-1)
    if (event.shiftKey && (document.activeElement === first || document.activeElement === dialog)) {
      event.preventDefault()
      last?.focus()
    } else if (!event.shiftKey && (document.activeElement === last || document.activeElement === dialog)) {
      event.preventDefault()
      first?.focus()
    }
  }

  onMount(() => {
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
    mounted = true
    if (typeof dialog.showModal === 'function') dialog.showModal()
    else dialog.setAttribute('open', '')
    nextButton?.focus({ preventScroll: true })
    if (typeof ResizeObserver !== 'undefined') {
      resizeObserver = new ResizeObserver(scheduleMeasure)
      resizeObserver.observe(card)
      resizeObserver.observe(scope)
    }
    const mutationObserver = new MutationObserver((records) => {
      if (records.some((record) => !dialog.contains(record.target))) scheduleMeasure()
    })
    mutationObserver.observe(scope, { childList: true, subtree: true, attributes: true, attributeFilter: ['class', 'style', 'hidden'] })
    window.addEventListener('resize', scheduleMeasure)
    window.addEventListener('scroll', scheduleMeasure, true)
    window.visualViewport?.addEventListener('resize', scheduleMeasure)
    measure()
    void tick().then(() => { if (mounted) revealStep(step) })
    return () => {
      mounted = false
      dismissed = true
      cancelAnimationFrame(frame)
      resizeObserver?.disconnect()
      mutationObserver.disconnect()
      window.removeEventListener('resize', scheduleMeasure)
      window.removeEventListener('scroll', scheduleMeasure, true)
      window.visualViewport?.removeEventListener('resize', scheduleMeasure)
      closeDialog()
      restoreScroll()
      if (previousFocus?.isConnected && !previousFocus.closest('[inert]')) previousFocus.focus({ preventScroll: true })
    }
  })
</script>

<dialog bind:this={dialog} class="spotlight-dialog" data-workbench-spotlight-tour aria-modal="true" aria-labelledby={`${id}-title`} aria-describedby={`${id}-body`}
  onkeydown={keydown} oncancel={(event) => { event.preventDefault(); dismiss('skip') }} onclose={() => { if (!dismissed) dismiss('skip') }}>
  <svg class="spotlight-scrim" aria-hidden="true" width="100%" height="100%">
    <path d={`M0,0H${viewport.width}V${viewport.height}H0Z${cutout}`} fill="rgb(3 7 18 / 0.66)" fill-rule="evenodd" />
  </svg>
  {#if targetRect}
    <div class="spotlight-ring" aria-hidden="true" style:left={`${targetRect.left}px`} style:top={`${targetRect.top}px`}
      style:width={`${targetRect.width}px`} style:height={`${targetRect.height}px`}></div>
  {/if}
  <div bind:this={card} class="spotlight-card" style:left={`${position.left}px`} style:top={`${position.top}px`} data-tour-step={step?.id}>
    <div class="tour-topline">
      <span>{labels.dialog}</span>
      <button type="button" class="tour-close" aria-label={labels.skip} onclick={() => dismiss('skip')}><X size={16} /></button>
    </div>
    <div class="tour-content">
      {#if portrait}<img class="tour-portrait" src={portrait} alt="" draggable="false" />{/if}
      <div class="tour-copy" aria-live="polite" aria-atomic="true">
        <p class="tour-counter">{labels.step(index, steps.length)}</p>
        <h2 id={`${id}-title`}>{step?.title ?? labels.dialog}</h2>
        <p id={`${id}-body`} class="tour-body">{step?.body ?? ''}</p>
      </div>
    </div>
    <div class="tour-progress" aria-hidden="true">
      {#each steps as item, position (item.id)}<span class:current={position === index} class:visited={position < index}></span>{/each}
    </div>
    <div class="tour-actions">
      <button type="button" class="tour-skip" data-tour-skip onclick={() => dismiss('skip')}>{labels.skip}</button>
      <button type="button" class="tour-back" data-tour-back disabled={index === 0} onclick={() => move(-1)}><ArrowLeft size={15} /><span>{labels.back}</span></button>
      <button bind:this={nextButton} type="button" class="tour-next" data-tour-next onclick={() => index === steps.length - 1 ? dismiss('complete') : move(1)}>
        <span>{index === steps.length - 1 ? labels.done : labels.next}</span>
        {#if index === steps.length - 1}<Check size={15} />{:else}<ArrowRight size={15} />{/if}
      </button>
    </div>
  </div>
</dialog>

<style>
  .spotlight-dialog { position: fixed; inset: 0; width: 100%; height: 100%; max-width: none; max-height: none; margin: 0; padding: 0; overflow: hidden; overscroll-behavior: contain; border: 0; background: transparent; color: var(--foreground); }
  .spotlight-dialog::backdrop { background: transparent; }
  .spotlight-scrim { position: absolute; inset: 0; pointer-events: none; }
  .spotlight-ring { position: absolute; pointer-events: none; outline: 2px solid color-mix(in oklab, var(--primary) 75%, white); outline-offset: 3px; border-radius: 8px; box-shadow: 0 0 0 7px color-mix(in oklab, var(--primary) 15%, transparent); }
  .spotlight-card { position: absolute; width: min(380px, calc(100vw - 32px)); max-height: calc(100dvh - 32px); overflow: auto; padding: 16px; border: 1px solid color-mix(in oklab, var(--primary) 30%, var(--border)); border-radius: 18px; background: var(--background); box-shadow: 0 18px 70px rgb(0 0 0 / .35); }
  .tour-topline { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 10px; color: var(--muted-foreground); font-size: 11px; font-weight: 500; }
  .tour-close { display: grid; place-items: center; width: 26px; height: 26px; padding: 0; border-radius: 6px; }
  .tour-content { display: flex; align-items: flex-start; gap: 14px; }
  .tour-portrait { width: 92px; height: 112px; flex: 0 0 92px; object-fit: contain; object-position: center top; filter: drop-shadow(0 6px 10px rgb(0 0 0 / .1)); }
  .tour-copy { min-width: 0; padding-top: 2px; }
  .tour-counter { margin: 0 0 7px; color: var(--primary); font-size: 11px; font-weight: 650; }
  h2 { margin: 0; font-size: 16px; font-weight: 650; line-height: 1.5; text-wrap: balance; }
  .tour-body { margin: 8px 0 0; font-size: 12px; line-height: 1.8; color: var(--muted-foreground); white-space: pre-line; overflow-wrap: anywhere; }
  .tour-progress { display: flex; gap: 5px; margin: 17px 0 14px; }
  .tour-progress span { height: 3px; flex: 1; border-radius: 999px; background: var(--muted); }
  .tour-progress .visited { background: color-mix(in oklab, var(--primary) 35%, var(--muted)); }
  .tour-progress .current { background: var(--primary); }
  .tour-actions { display: flex; align-items: center; gap: 8px; }
  button { cursor: pointer; border: 0; background: transparent; color: inherit; font: inherit; }
  button:focus-visible { outline: 2px solid var(--ring); outline-offset: 3px; }
  .tour-close:hover, .tour-skip:hover, .tour-back:hover:not(:disabled) { background: var(--accent); }
  .tour-skip, .tour-back, .tour-next { display: flex; align-items: center; justify-content: center; gap: 5px; min-height: 34px; border-radius: 7px; padding: 7px 10px; font-size: 12px; white-space: nowrap; }
  .tour-skip { margin-right: auto; padding-inline: 7px; color: var(--muted-foreground); }
  .tour-back { border: 1px solid var(--border); }
  .tour-back:disabled { opacity: .35; cursor: default; }
  .tour-next { color: var(--primary-foreground); background: var(--primary); font-weight: 550; }
  .tour-next:hover { filter: brightness(1.07); }
  @media (max-width: 420px) {
    .spotlight-card { padding: 14px; }
    .tour-content { gap: 10px; }
    .tour-portrait { width: 66px; height: 90px; flex-basis: 66px; }
    .tour-actions { gap: 5px; }
    .tour-back, .tour-next { padding-inline: 8px; }
  }
</style>

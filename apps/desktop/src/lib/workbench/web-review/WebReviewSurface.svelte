<script lang="ts">
  import { onMount } from 'svelte'
  import { createWebReviewFrame } from './webReviewFrame'
  import { reviewPageUrl, type WebReviewFocus, type WebReviewFrameState, type WebReviewRect, type WebReviewSelection } from './webReviewProtocol'

  export let url: string
  export let viewport: Readonly<{ width: number; height: number }>
  export let mode: 'browse' | 'select' = 'browse'
  export let annotations: readonly Readonly<{ id: string; page_url: string; element: { selector: string } }>[] = []
  export let selectedId: string | null = null
  export let focusTarget: WebReviewFocus | null = null
  export let refreshKey = 0
  export let onSelect: (selection: WebReviewSelection) => void
  export let onStatus: (state: WebReviewFrameState) => void
  export let onAnnotationClick: (id: string) => void = () => undefined
  export let onSelectionRect: (rect: WebReviewRect | null) => void = () => undefined
  export let onFocusMissing: (reason: 'page_changed' | 'element_missing') => void = () => undefined

  let root: HTMLDivElement
  let frame: HTMLIFrameElement
  let controller: ReturnType<typeof createWebReviewFrame> | undefined
  let mounted = false
  let currentUrl = ''
  let currentRefreshKey = -1
  let lastFocusSequence: number | null = null
  let anchor: WebReviewRect | null = null
  function updateAnchor() {
    if (!anchor || !root || !frame) { onSelectionRect(null); return }
    const rootRect = root.getBoundingClientRect()
    const frameRect = frame.getBoundingClientRect()
    const shifted = { ...anchor, x: anchor.x + frameRect.x - rootRect.x, y: anchor.y + frameRect.y - rootRect.y }
    onSelectionRect(shifted.x + shifted.width < 0 || shifted.y + shifted.height < 0 || shifted.x > root.clientWidth || shifted.y > root.clientHeight ? null : shifted)
  }
  function load(nextUrl: string, nextRefresh: number) {
    controller?.dispose()
    currentUrl = nextUrl
    currentRefreshKey = nextRefresh
    lastFocusSequence = null
    anchor = null
    controller = createWebReviewFrame({ iframe: frame, url: nextUrl, onState: (state) => onStatus(state),
      onSelection: (selection) => onSelect(selection), onAnnotationClick: (id) => onAnnotationClick(id),
      onAnchor: (rect) => { anchor = rect; updateAnchor() }, onFocusMissing: (reason) => onFocusMissing(reason) })
    controller.setMode(mode)
    controller.setAnnotations(annotations.map((item, index) => ({ id: item.id, number: index + 1, selector: item.element.selector, page_url: item.page_url })), selectedId)
    const safeUrl = reviewPageUrl(nextUrl)
    if (safeUrl) frame.src = safeUrl
    else frame.removeAttribute('src')
    controller.connect()
  }
  onMount(() => {
    mounted = true
    const observer = new ResizeObserver(updateAnchor)
    observer.observe(root)
    return () => { mounted = false; observer.disconnect(); controller?.dispose() }
  })
  $: if (mounted && (currentUrl !== url || currentRefreshKey !== refreshKey)) load(url, refreshKey)
  $: controller?.setMode(mode)
  $: controller?.setAnnotations(annotations.map((item, index) => ({ id: item.id, number: index + 1, selector: item.element.selector, page_url: item.page_url })), selectedId)
  $: if (controller && focusTarget && lastFocusSequence !== focusTarget.sequence) {
    lastFocusSequence = focusTarget.sequence
    controller.focus(focusTarget)
  }
</script>

<div bind:this={root} class="web-review-surface" data-web-review-surface onscroll={updateAnchor}>
  <iframe bind:this={frame} title="Web page under review" referrerpolicy="no-referrer"
    sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-downloads allow-modals"
    style:width={`${viewport.width}px`} style:height={`${viewport.height}px`}></iframe>
</div>

<style>
  .web-review-surface { width: 100%; height: 100%; min-height: 280px; overflow: auto; position: relative; background: var(--muted); }
  iframe { display: block; flex-shrink: 0; border: 0; background: white; }
</style>

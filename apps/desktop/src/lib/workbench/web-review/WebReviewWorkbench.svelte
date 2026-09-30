<script lang="ts">
  import { tick } from 'svelte'
  import { Globe, MousePointer2, Hand, RotateCw, Monitor, Smartphone, MessageSquare, ExternalLink } from '@lucide/svelte'
  import type { WebReviewAnnotation, WebReviewData } from '../../generated/feedback'
  import { locale } from '../../preferences'
  import { unavailableVoiceInputState, useVoiceInput } from '../../speech/voiceInputContext'
  import type { SpeechTarget } from '../../speech/speechDraftQueue'
  import WebReviewSurface from './WebReviewSurface.svelte'
  import WebReviewCommentCard from './WebReviewCommentCard.svelte'
  import WebReviewComments from './WebReviewComments.svelte'
  import { capturedElementRect, emptyWebReviewState, prepareElementAnnotation, validateWebReviewState, type WebReviewState } from './reviewModel'
  import { webReviewText } from './reviewI18n'
  import type { WebReviewFrameState, WebReviewSelection } from './webReviewProtocol'

  export let data: WebReviewData
  export let state: WebReviewState | null = null
  export let disabled = false
  export let readOnly = false
  export let onChange: (state: WebReviewState) => void
  let viewport = { ...data.viewport }
  let pageUrl = data.url
  let mode: 'browse' | 'select' = 'browse'
  let refreshKey = 0
  let selectedId: string | null = null
  let commentsOpen = false
  let selectionMessage = ''
  let focusTarget: { selector: string; page_url: string; sequence: number } | null = null
  let focusSequence = 0
  let selectedComment: WebReviewCommentCard | undefined
  let revealedSequence: number | undefined
  let frameState: WebReviewFrameState = { status: 'loading', page_url: data.url, page_title: '', viewport: null }
  const voiceState = useVoiceInput()?.state ?? unavailableVoiceInputState
  const tr = (source: string) => webReviewText($locale, source)
  $: current = state ?? emptyWebReviewState()
  $: selected = current.annotations.find((item) => item.id === selectedId)
  $: issue = validateWebReviewState(data, current)
  $: connected = frameState.status === 'ready'
  $: void revealVoiceTarget($voiceState.revealSequence, $voiceState.revealTarget, current)

  function update(next: WebReviewState) { if (!disabled && !readOnly) { state = next; current = next; onChange(next) } }
  async function selectElement(selection: WebReviewSelection) {
    if (disabled || readOnly || mode !== 'select') return
    const prepared = prepareElementAnnotation(current, {
      page_url: selection.page_url, viewport: { width: selection.viewport.width, height: selection.viewport.height },
      element: { selector: selection.selector, tag_name: selection.tag_name, text: selection.text,
        rect: capturedElementRect(selection.rect) },
    }, () => `note-${crypto.randomUUID()}`, selectedId)
    if (!prepared) { selectionMessage = tr('Comment limit reached.'); return }
    selectionMessage = ''
    if (prepared.created) update(prepared.state)
    selectedId = prepared.annotation.id
    commentsOpen = false
    await tick()
    selectedComment?.focusComment()
  }
  async function openComment(annotation: WebReviewAnnotation, focus = true) {
    selectedId = annotation.id
    commentsOpen = false
    if (!readOnly) {
      viewport = { ...annotation.viewport }
      if (annotation.page_url !== frameState.page_url) {
        pageUrl = annotation.page_url
        // A SPA can leave the iframe's src unchanged while navigating elsewhere.
        // Returning to that src still needs an explicit navigation.
        refreshKey += 1
      }
      focusTarget = { selector: annotation.element.selector, page_url: annotation.page_url, sequence: ++focusSequence }
    }
    await tick()
    if (focus && !disabled && !readOnly) selectedComment?.focusComment()
  }
  function changeComment(annotation: WebReviewAnnotation) { update({ ...current, annotations: current.annotations.map((item) => item.id === annotation.id ? annotation : item) }) }
  function deleteComment(id: string) {
    if (disabled || readOnly) return
    update({ ...current, annotations: current.annotations.filter((item) => item.id !== id) })
    if (selectedId === id) selectedId = null
  }
  function changeViewport(next: { width: number; height: number }) {
    viewport = next
    focusTarget = null
    selectedId = null
  }
  function refreshPage() {
    pageUrl = frameState.page_url
    refreshKey += 1
    frameState = { ...frameState, status: 'loading' }
  }
  async function revealVoiceTarget(sequence: number | undefined, target: SpeechTarget | null | undefined, review: WebReviewState) {
    if (sequence === undefined || sequence === revealedSequence || target?.requestId !== $voiceState.requestId || target?.destination.kind !== 'web_review_annotation') return
    const destination = target.destination
    const annotation = review.annotations.find((item) => item.id === destination.annotationId)
    if (!annotation) return
    revealedSequence = sequence
    await openComment(annotation)
  }
</script>

<section class="web-review flex min-h-0 min-w-0 flex-col gap-3" class:web-review-readonly={readOnly} aria-label={tr('Web review')}>
  <header class="flex flex-wrap items-center gap-3">
    <Globe class="size-5 shrink-0 text-primary" />
    <div class="min-w-0 flex-1"><h2 class="m-0 truncate text-base font-semibold">{data.title}</h2><p class="m-0 mt-1 truncate text-xs text-muted-foreground" title={data.url}>{data.url} · {data.source_version}</p></div>
    <a href={data.url} target="_blank" rel="noreferrer noopener" class="inline-flex items-center gap-1 rounded-lg border px-3 py-2 text-xs"><ExternalLink class="size-3.5" />{tr('Open page')}</a>
  </header>
  {#if readOnly}
    <p class="m-0 text-xs text-muted-foreground">{tr('Read only')} · {tr('Submitted comments keep their captured context.')}</p>
    {#if !current.annotations.length}<p class="m-0 text-sm text-muted-foreground">{tr('No element comments yet.')}</p>{/if}
    <div class="grid gap-3">
      {#each current.annotations as annotation, index (annotation.id)}
        <WebReviewCommentCard {annotation} number={index + 1} disabled={true} collapsible={false} onUpdate={() => {}} onDelete={() => {}} onCollapse={() => {}} />
      {/each}
    </div>
  {:else}
    <div class="flex flex-wrap items-center gap-2" aria-label={tr('Web review')}>
      <div class="inline-flex overflow-hidden rounded-lg border">
        <button type="button" aria-label={tr('Browse')} aria-pressed={mode === 'browse'} {disabled} onclick={() => mode = 'browse'} class="inline-flex items-center gap-1.5 px-3 py-2 text-xs aria-pressed:bg-primary/10 aria-pressed:text-primary disabled:opacity-40"><Hand class="size-3.5" />{tr('Browse')}</button>
        <button type="button" aria-label={tr('Select elements')} aria-pressed={mode === 'select'} disabled={disabled || !connected} onclick={() => mode = 'select'} class="inline-flex items-center gap-1.5 border-l px-3 py-2 text-xs aria-pressed:bg-primary/10 aria-pressed:text-primary disabled:opacity-40"><MousePointer2 class="size-3.5" />{tr('Select elements')}</button>
      </div>
      <button type="button" {disabled} aria-label={tr('Refresh page')} onclick={refreshPage} class="rounded-lg border p-2"><RotateCw class="size-3.5" /></button>
      <button type="button" {disabled} aria-label={tr('Desktop viewport')} aria-pressed={viewport.width >= 768} onclick={() => changeViewport({ width: 1440, height: 900 })} class="inline-flex items-center gap-1 rounded-lg border px-2 py-2 text-xs aria-pressed:bg-muted"><Monitor class="size-3.5" />1440</button>
      <button type="button" {disabled} aria-label={tr('Mobile viewport')} aria-pressed={viewport.width < 768} onclick={() => changeViewport({ width: 390, height: 844 })} class="inline-flex items-center gap-1 rounded-lg border px-2 py-2 text-xs aria-pressed:bg-muted"><Smartphone class="size-3.5" />390</button>
      <span class="text-[11px] tabular-nums text-muted-foreground">{viewport.width} × {viewport.height}</span>
      <button type="button" aria-pressed={commentsOpen} onclick={() => commentsOpen = !commentsOpen} class="ml-auto inline-flex items-center gap-1.5 rounded-lg border px-3 py-2 text-xs aria-pressed:bg-primary/10"><MessageSquare class="size-3.5" />{tr('Review comments')} · {current.annotations.length}</button>
    </div>
    <div role="status" class="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground" data-web-review-status={frameState.status}>
      <span class:text-emerald-700={connected} class:dark:text-emerald-400={connected}>{tr(connected ? 'Element selection connected' : frameState.status === 'loading' ? 'Connecting…' : 'Preview only')}</span>
      {#if frameState.status === 'unavailable'}<span>{tr(frameState.reason === 'bridge_missing' ? 'This page needs the RambleDesk review bridge to select elements.' : 'Page unavailable')} {tr(frameState.reason === 'bridge_missing' ? 'You can browse the preview and add screenshots or overall feedback.' : 'Open the page separately and add screenshots or overall feedback.')}</span>{/if}
    </div>
    <div class="web-review-stage relative min-h-0 min-w-0">
      <WebReviewSurface url={pageUrl} {viewport} mode={disabled ? 'browse' : mode} annotations={current.annotations} {selectedId} {focusTarget} {refreshKey}
        onSelect={(selection) => void selectElement(selection)}
        onStatus={(status) => frameState = status}
        onFocusMissing={() => selectionMessage = tr('The saved element is unavailable. Its captured context is preserved.')}
        onAnnotationClick={(id) => { const annotation = current.annotations.find((item) => item.id === id); if (annotation) void openComment(annotation) }} />
      {#if selected}
        <div class="web-review-comment-dock z-20" data-web-review-comment-dock>
          {#key selected.id}<WebReviewCommentCard bind:this={selectedComment} annotation={selected} number={current.annotations.findIndex((item) => item.id === selected.id) + 1} {disabled} floating={true} onUpdate={changeComment} onDelete={() => deleteComment(selected.id)} onCollapse={() => selectedId = null} />{/key}
        </div>
      {/if}
      {#if commentsOpen}<div class="absolute inset-y-3 right-3 z-30 w-[320px] max-w-[calc(100%-24px)]"><WebReviewComments annotations={current.annotations} {selectedId} onSelect={(annotation) => void openComment(annotation)} onClose={() => commentsOpen = false} /></div>{/if}
    </div>
    <p class="m-0 text-[11px] leading-5 text-muted-foreground">{tr('Comments are saved with your overall feedback.')}</p>
    {#if selectionMessage || issue}<p role="status" class="m-0 text-xs text-amber-700 dark:text-amber-400">{selectionMessage || tr(issue ?? '')}</p>{/if}
  {/if}
</section>

<style>
  .web-review { flex: 1; min-height: 0; padding: 12px; }
  .web-review:not(.web-review-readonly) { height: 100%; overflow: hidden; }
  .web-review-stage { flex: 1; height: auto; min-height: 0; overflow: hidden; }
  .web-review-stage :global(.web-review-surface) { min-height: 0; }
  .web-review-comment-dock {
    position: absolute;
    left: 50%;
    bottom: 12px;
    transform: translateX(-50%);
    width: 480px;
    height: 320px;
    max-width: calc(100% - 24px);
    max-height: max(0px, calc(100% - 24px));
    overflow: hidden;
  }
</style>

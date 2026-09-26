<script lang="ts">
  import { tick } from 'svelte'
  import { FileText, MessageSquare, PencilLine, LockKeyhole, Check, X } from '@lucide/svelte'
  import type { DocumentReviewData, ParagraphMark, ReviewAnnotation } from '../../generated/feedback'
  import { locale } from '../../preferences'
  import { annotatedSegments, changeParagraphMark, emptyReviewState, validateReviewState, type DocumentReviewState, type ReviewAnchor } from './reviewModel'
  import { selectionAnchor } from './reviewSelection'
  import { reviewText } from './reviewI18n'
  import ReviewCommentCard from './ReviewCommentCard.svelte'

  export let data: DocumentReviewData
  export let state: DocumentReviewState | null = null
  export let disabled = false
  export let onChange: (state: DocumentReviewState) => void
  let root: HTMLElement
  let tab: 'original' | 'comments' = 'original'
  let filter: 'all' | 'open' | 'resolved' = 'open'
  let selectedId: string | null = null
  let selectedText: ReviewAnchor | null = null
  let selectionMessage = ''
  $: current = state ?? emptyReviewState()
  $: visibleComments = current.annotations.filter((item) => filter === 'all' || item.status === filter)
  $: openCount = current.annotations.filter((item) => item.status === 'open').length
  $: issue = validateReviewState(data, current)
  const tr = (text: string) => reviewText($locale, text)
  function update(next: DocumentReviewState) { if (!disabled) { current = next; state = next; onChange(next) } }
  function captureSelection() {
    if (!root || disabled) return
    const anchor = selectionAnchor(root, document.getSelection())
    if (anchor === 'cross_paragraph') { selectedText = null; selectionMessage = tr('Select text within one paragraph. Use paragraph comments for broader feedback.') }
    else if (anchor) { selectedText = anchor; selectionMessage = '' }
    else if (!root.querySelector('[data-review-selection-toolbar]')?.contains(document.activeElement)) { selectedText = null; selectionMessage = '' }
  }
  async function navigateTabs(event: KeyboardEvent) {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return
    event.preventDefault()
    tab = event.key === 'Home' ? 'original' : event.key === 'End' ? 'comments' : tab === 'original' ? 'comments' : 'original'
    await tick()
    root.querySelector<HTMLElement>(`#review-${tab}-tab`)?.focus()
  }
  async function addComment(anchor: ReviewAnchor, kind: ReviewAnnotation['kind']) {
    if (disabled) return
    if (current.annotations.length >= 500) { selectionMessage = tr('Comment limit reached.'); return }
    const annotation: ReviewAnnotation = { ...anchor, id: `note-${crypto.randomUUID()}`, kind, body: '', replacement: kind === 'suggestion' ? anchor.quote ?? data.paragraphs.find((item) => item.id === anchor.paragraph_id)?.text ?? '' : null, status: 'open' }
    selectedId = annotation.id
    selectedText = null
    document.getSelection()?.removeAllRanges()
    update({ ...current, annotations: [...current.annotations, annotation] })
    await locate(annotation)
  }
  function changeComment(annotation: ReviewAnnotation) { update({ ...current, annotations: current.annotations.map((item) => item.id === annotation.id ? annotation : item) }) }
  function deleteComment(id: string) { update({ ...current, annotations: current.annotations.filter((item) => item.id !== id) }); if (selectedId === id) selectedId = null }
  async function locate(annotation: ReviewAnnotation) {
    tab = 'original'; selectedId = annotation.id
    await tick()
    root?.querySelector<HTMLElement>(`[data-paragraph-id="${annotation.paragraph_id}"]`)?.scrollIntoView({ block: 'nearest', behavior: 'smooth' })
  }
  function paragraphLabel(id: string) {
    const index = data.paragraphs.findIndex((paragraph) => paragraph.id === id)
    return data.paragraphs[index]?.label ?? `${tr('Paragraph')} ${index + 1}`
  }
  function activateHighlight(event: MouseEvent) {
    if (document.getSelection()?.toString()) return
    const target = (event.target as HTMLElement).closest<HTMLElement>('[data-annotation-ids]')
    if (target?.dataset.annotationIds) selectedId = target.dataset.annotationIds.split(' ')[0]
  }
</script>

<svelte:document onselectionchange={captureSelection} />

<section bind:this={root} class="document-review flex min-h-0 min-w-0 flex-col" aria-label={tr('Document review')}>
  <header class="border-b pb-4">
    <div class="flex items-start gap-3">
      <span class="mt-0.5 rounded-lg border bg-muted/50 p-2 text-muted-foreground"><FileText class="size-5" /></span>
      <div class="min-w-0 flex-1">
        <h2 class="m-0 break-words text-lg font-semibold leading-7">{data.title}</h2>
        <p class="m-0 mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground"><span>{data.source_version}</span><span class="inline-flex items-center gap-1"><LockKeyhole class="size-3" />{tr('Original text is preserved')}</span></p>
      </div>
    </div>
    <div class="mt-4 flex flex-wrap items-center gap-3">
      <div class="inline-flex gap-1 rounded-lg bg-muted/60 p-1" role="tablist" aria-label={tr('Document review')}>
        <button type="button" role="tab" id="review-original-tab" aria-controls="review-original" aria-selected={tab === 'original'} tabindex={tab === 'original' ? 0 : -1} onkeydown={navigateTabs} onclick={() => tab = 'original'} class={`rounded-md px-3 py-1.5 text-xs font-medium ${tab === 'original' ? 'bg-background shadow-sm' : 'text-muted-foreground'}`}>{tr('Original')}</button>
        <button type="button" role="tab" id="review-comments-tab" aria-controls="review-comments" aria-selected={tab === 'comments'} tabindex={tab === 'comments' ? 0 : -1} onkeydown={navigateTabs} onclick={() => tab = 'comments'} class={`inline-flex items-center gap-2 rounded-md px-3 py-1.5 text-xs font-medium ${tab === 'comments' ? 'bg-background shadow-sm' : 'text-muted-foreground'}`}>{tr('All comments')}<span class="rounded bg-muted px-1.5 text-[10px]">{current.annotations.length}</span></button>
      </div>
      <span class="ml-auto text-xs text-muted-foreground" aria-label={tr('Review progress')}>{tr('Paragraphs marked')} {current.paragraph_marks.length}/{data.paragraphs.length}</span>
    </div>
  </header>

  {#if selectedText && !disabled && tab === 'original'}
    <div class="pointer-events-none sticky top-0 z-10 h-0">
    <div data-review-selection-toolbar class="pointer-events-auto absolute inset-x-0 top-2 flex flex-wrap items-center gap-2 rounded-lg border border-primary/30 bg-background p-3 shadow-sm" aria-label={tr('Selected passage')}>
      <p class="m-0 min-w-0 flex-1 truncate text-xs text-muted-foreground">“{selectedText.quote}”</p>
      <button type="button" onpointerdown={(event) => event.preventDefault()} onclick={() => selectedText && void addComment(selectedText, 'comment')} class="inline-flex items-center gap-1 rounded-md bg-primary px-3 py-2 text-xs text-primary-foreground"><MessageSquare class="size-3.5" />{tr('Comment')}</button>
      <button type="button" onpointerdown={(event) => event.preventDefault()} onclick={() => selectedText && void addComment(selectedText, 'suggestion')} class="inline-flex items-center gap-1 rounded-md border px-3 py-2 text-xs"><PencilLine class="size-3.5" />{tr('Suggest rewrite')}</button>
      <button type="button" aria-label={tr('Clear selection')} onclick={() => { selectedText = null; document.getSelection()?.removeAllRanges() }} class="rounded p-1"><X class="size-4" /></button>
    </div>
    </div>
  {/if}
  {#if selectionMessage}<p role="status" class="my-3 text-xs text-amber-700 dark:text-amber-400">{selectionMessage}</p>{/if}

  {#if tab === 'original'}
    <div id="review-original" role="tabpanel" aria-labelledby="review-original-tab" tabindex="0" class="py-4">
      <p class="mb-5 text-xs text-muted-foreground">{tr('Select text to comment or suggest a rewrite.')}</p>
      <div class="grid gap-6">
        {#each data.paragraphs as paragraph, index (paragraph.id)}
          {@const annotations = current.annotations.filter((item) => item.paragraph_id === paragraph.id)}
          {@const selected = annotations.find((item) => item.id === selectedId)}
          {@const mark = current.paragraph_marks.find((item) => item.paragraph_id === paragraph.id)}
          <article data-paragraph-id={paragraph.id} class="review-paragraph min-w-0 rounded-lg border-l-2 pl-4" class:border-emerald-500={mark?.decision === 'keep'} class:border-amber-500={mark?.decision === 'revise'} class:border-rose-500={mark?.decision === 'remove'} class:border-transparent={!mark}>
            <div class="mb-2 flex flex-wrap items-center gap-2 text-xs">
              <span class="font-mono text-[10px] tabular-nums text-muted-foreground">{String(index + 1).padStart(2, '0')}</span>
              {#if paragraph.label}<span class="min-w-0 flex-1 truncate font-medium text-muted-foreground">{paragraph.label}</span>{/if}
              <select value={mark?.decision ?? ''} {disabled} aria-label={`${tr('Mark paragraph')} ${index + 1}`} onchange={(event) => update(changeParagraphMark(current, paragraph.id, event.currentTarget.value as ParagraphMark['decision'] | ''))} class="ml-auto min-w-0 rounded-md border bg-background px-2 py-1 text-xs text-muted-foreground disabled:opacity-60">
                <option value="">{tr('Not marked')}</option><option value="keep">{tr('Keep')}</option><option value="revise">{tr('Revise')}</option><option value="remove">{tr('Remove')}</option>
              </select>
              <button type="button" {disabled} aria-label={`${tr('Comment on paragraph')} ${index + 1}`} title={tr('Paragraph comment')} onclick={() => void addComment({ paragraph_id: paragraph.id, start: null, end: null, quote: null }, 'comment')} class="rounded-md p-1.5 text-muted-foreground hover:bg-muted disabled:opacity-40"><MessageSquare class="size-3.5" /></button>
            </div>
            <div role="group" aria-label={paragraphLabel(paragraph.id)}>
              <p data-review-text={paragraph.id} class="review-source m-0 whitespace-pre-wrap break-words text-[15px] leading-8">{#each annotatedSegments(paragraph.text, annotations) as segment}{#if segment.annotationIds.length}<span role="button" tabindex="0" data-annotation-ids={segment.annotationIds.join(' ')} onclick={activateHighlight} onkeydown={(event) => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); selectedId = segment.annotationIds[0] } }} class={`cursor-pointer rounded-sm border-b-2 text-inherit focus-visible:outline-ring ${segment.annotationIds.includes(selectedId ?? '') ? 'border-primary bg-primary/20' : segment.open ? 'border-amber-400 bg-amber-300/20' : 'border-muted-foreground/30 bg-muted'}`}>{segment.text}</span>{:else}{segment.text}{/if}{/each}</p>
            </div>
            {#if annotations.length}
              <div class="mt-2 flex flex-wrap gap-1.5">
                {#each annotations as annotation, noteIndex (annotation.id)}
                  <button type="button" aria-pressed={selectedId === annotation.id} onclick={() => selectedId = selectedId === annotation.id ? null : annotation.id} class="inline-flex items-center gap-1 rounded-full border px-2 py-1 text-[11px] text-muted-foreground hover:bg-muted aria-pressed:border-primary aria-pressed:text-primary"><MessageSquare class="size-3" />{noteIndex + 1}{#if annotation.status === 'resolved'}<Check class="size-3" />{/if}{#if !annotation.body.trim()}<span class="text-amber-600">•</span>{/if}</button>
                {/each}
              </div>
            {/if}
            {#if selected}
              <div class="mt-3"><ReviewCommentCard annotation={selected} paragraphLabel={paragraphLabel(paragraph.id)} {disabled} contextual onUpdate={changeComment} onDelete={() => deleteComment(selected.id)} onLocate={() => void locate(selected)} onClose={() => selectedId = null} /></div>
            {/if}
          </article>
        {/each}
      </div>
    </div>
  {:else}
    <div id="review-comments" role="tabpanel" aria-labelledby="review-comments-tab" tabindex="0" class="py-4">
      <div class="mb-4 flex items-center gap-2">
        {#each ['open', 'resolved', 'all'] as value}<button type="button" aria-pressed={filter === value} onclick={() => filter = value as typeof filter} class="rounded-full border px-3 py-1.5 text-xs aria-pressed:border-primary aria-pressed:bg-primary/5 aria-pressed:text-primary">{tr(value === 'open' ? 'Open' : value === 'resolved' ? 'Resolved' : 'All')}{value === 'open' ? ` ${openCount}` : ''}</button>{/each}
      </div>
      <div class="grid gap-3">
        {#each visibleComments as annotation (annotation.id)}<ReviewCommentCard {annotation} paragraphLabel={paragraphLabel(annotation.paragraph_id)} {disabled} onUpdate={changeComment} onDelete={() => deleteComment(annotation.id)} onLocate={() => void locate(annotation)} />{:else}
          <div class="rounded-xl border border-dashed px-6 py-10 text-center"><MessageSquare class="mx-auto mb-3 size-6 text-muted-foreground" /><p class="text-sm font-medium">{tr(current.annotations.length ? 'No comments in this view' : 'No comments yet')}</p><p class="mx-auto mt-2 max-w-sm text-xs leading-6 text-muted-foreground">{tr('Select a passage or use a paragraph’s comment button to begin.')}</p><button type="button" onclick={() => tab = 'original'} class="mt-3 text-xs underline">{tr('View original')}</button></div>
        {/each}
      </div>
    </div>
  {/if}

  <footer class="mt-3 border-t pt-4">
    <fieldset {disabled} class="m-0 border-0 p-0">
      <legend class="mb-2 text-xs font-medium">{tr('Review decision')}</legend>
      <div class="flex flex-wrap gap-2">
        <button type="button" {disabled} aria-pressed={current.verdict === 'ready'} onclick={() => update({ ...current, verdict: current.verdict === 'ready' ? null : 'ready' })} class="rounded-lg border px-3 py-2 text-xs aria-pressed:border-emerald-600 aria-pressed:bg-emerald-500/10 disabled:opacity-60">{tr('Ready as written')}</button>
        <button type="button" {disabled} aria-pressed={current.verdict === 'changes_requested'} onclick={() => update({ ...current, verdict: current.verdict === 'changes_requested' ? null : 'changes_requested' })} class="rounded-lg border px-3 py-2 text-xs aria-pressed:border-amber-600 aria-pressed:bg-amber-500/10 disabled:opacity-60">{tr('Changes requested')}</button>
      </div>
    </fieldset>
    <p class="mb-0 mt-3 text-xs leading-5 text-muted-foreground">{tr('Choose a decision, then submit with your overall feedback.')}</p>
    {#if current.verdict && issue}<p role="status" class="mt-2 text-xs text-amber-700 dark:text-amber-400">{tr(issue)}</p>{/if}
  </footer>
</section>

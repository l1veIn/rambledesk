<script lang="ts">
  import { tick } from 'svelte'
  import { FileText, MessageSquare, PencilLine, LockKeyhole, X } from '@lucide/svelte'
  import type { DocumentReviewData, ReviewAnnotation } from '../../generated/feedback'
  import { locale } from '../../preferences'
  import { emptyReviewState, prepareAnnotation, validateReviewState, type DocumentReviewState, type ReviewAnchor } from './reviewModel'
  import { parseReviewMarkdown } from './reviewMarkdown'
  import { selectionAnchor } from './reviewSelection'
  import { reviewText } from './reviewI18n'
  import ReviewMarkdown from './ReviewMarkdown.svelte'
  import ReviewCommentCard from './ReviewCommentCard.svelte'
  import { unavailableVoiceInputState, useVoiceInput } from '../../speech/voiceInputContext'
  import type { SpeechTarget } from '../../speech/speechDraftQueue'
  import { fieldAttachmentText } from '../../input/fieldAttachmentText'
  import { unavailableInputToolsState, useInputTools } from '../../input/inputToolsContext'

  export let data: DocumentReviewData
  export let state: DocumentReviewState | null = null
  export let disabled = false
  export let onChange: (state: DocumentReviewState) => void
  const voiceState = useVoiceInput()?.state ?? unavailableVoiceInputState
  const inputToolsState = useInputTools()?.state ?? unavailableInputToolsState
  const whole: ReviewAnchor = { start: null, end: null, quote: null }
  let root: HTMLElement, sourceRoot: HTMLElement
  let rootWidth = 0, cardTop = 0, cardHeight = 0
  let floatingCard = false
  let selectedId: string | null = null
  let selectedComment: ReviewCommentCard | undefined
  let selectedText: ReviewAnchor | null = null
  let selectionMessage = '', selectionPreview = ''
  let revealedSequence: number | undefined
  const untouchedSuggestions = new Set<string>()
  $: current = state ?? emptyReviewState()
  $: model = parseReviewMarkdown(data.markdown)
  $: issue = validateReviewState(data, current)
  $: selected = current.annotations.find((note) => note.id === selectedId)
  $: groups = groupNotes(model, current.annotations)
  $: rewriteAllowed = selectedText?.quote !== null && [...(selectedText?.quote ?? '')].length <= 8000
  $: void positionCard(selectedId, rootWidth, current.annotations)
  $: void revealVoiceTarget($voiceState.revealSequence, $voiceState.revealTarget, current)
  const tr = (text: string) => reviewText($locale, text)
  function update(next: DocumentReviewState) { if (!disabled) { current = next; state = next; onChange(next) } }
  function blockIndex(note: ReviewAnnotation) {
    if (note.start === null) return -1
    let low = 0, high = model.nodes.length
    while (low < high) { const mid = (low + high) >>> 1; if (note.start < (model.nodes[mid].sourceEnd ?? Infinity)) high = mid; else low = mid + 1 }
    return Math.min(low, model.nodes.length - 1)
  }
  function groupNotes(document: typeof model, annotations: ReviewAnnotation[]) {
    const result = [-1, ...document.nodes.map((_, index) => index)].map((index) => ({ index, node: document.nodes[index], notes: [] as ReviewAnnotation[] }))
    for (const annotation of annotations) result[blockIndex(annotation) + 1].notes.push(annotation)
    return result
  }
  function captureSelection() {
    if (!sourceRoot || disabled) return
    // Retain the verified source snapshot when a toolbar button takes focus.
    if (root.querySelector('[data-review-selection-toolbar]')?.contains(document.activeElement)) return
    const anchor = selectionAnchor(sourceRoot, document.getSelection(), model)
    if (anchor === 'unmapped') { selectedText = null; selectionMessage = tr('Select text without image placeholders. You can also comment on the whole document.') }
    else if (anchor) { selectedText = anchor; selectionPreview = anchor.quote ?? ''; selectionMessage = '' }
    else { selectedText = null; selectionMessage = '' }
  }
  async function addComment(anchor: ReviewAnchor, kind: ReviewAnnotation['kind']) {
    if (disabled) return
    const prepared = prepareAnnotation(data, current, anchor, kind, () => `note-${crypto.randomUUID()}`)
    if (!prepared) { selectionMessage = tr('Comment limit reached.'); return }
    if (kind === 'suggestion') untouchedSuggestions.add(prepared.annotation.id)
    selectedText = null; selectionMessage = ''; document.getSelection()?.removeAllRanges()
    update(prepared.state)
    await openComment(prepared.annotation)
  }
  function changeComment(annotation: ReviewAnnotation) {
    const previous = current.annotations.find((item) => item.id === annotation.id)
    if (previous && (previous.body !== annotation.body || previous.replacement !== annotation.replacement)) untouchedSuggestions.delete(annotation.id)
    update({ ...current, annotations: current.annotations.map((item) => item.id === annotation.id ? annotation : item) })
  }
  async function deleteComment(id: string) {
    if (disabled) return
    update({ ...current, annotations: current.annotations.filter((item) => item.id !== id) })
    if (selectedId === id) selectedId = null
    await tick()
    root?.querySelector<HTMLElement>('[data-tour="review-comment"] button')?.focus({ preventScroll: true })
  }
  async function positionCard(id: string | null, width: number, _notes: ReviewAnnotation[]) {
    await tick()
    if (!id || id !== selectedId || !sourceRoot) { cardTop = 0; floatingCard = false; return }
    const anchor = sourceRoot.querySelector<HTMLElement>(`[data-annotation-ids~="${id}"]`)
    const sourceBlock = anchor?.closest('[data-review-block]')?.querySelector('.review-source-body')
    const anchorRect = anchor?.getBoundingClientRect()
    floatingCard = width < 900 && !!anchorRect && !!sourceBlock && sourceBlock.getBoundingClientRect().bottom - anchorRect.bottom > 240
    cardTop = anchorRect && (width >= 900 || floatingCard)
      ? Math.max(0, (floatingCard ? anchorRect.bottom + 12 : anchorRect.top) - sourceRoot.getBoundingClientRect().top) : 0
  }
  async function showComment(annotation: ReviewAnnotation) {
    selectedId = annotation.id
    await tick()
    await positionCard(annotation.id, rootWidth, current.annotations)
  }
  async function openComment(annotation: ReviewAnnotation) {
    await showComment(annotation)
    if (!disabled && selectedId === annotation.id) selectedComment?.focusComment()
  }
  function selectHighlight(id: string) {
    const annotation = current.annotations.find((item) => item.id === id)
    if (annotation) void openComment(annotation)
  }
  function locateSelection(id = selectedId, scroll = true, stableLeaf = false) {
    const highlight = sourceRoot?.querySelector<HTMLElement>(`[data-annotation-ids~="${id}"]`)
    const target = (stableLeaf ? highlight?.closest<HTMLElement>('[data-review-leaf]') : highlight)
      ?? root?.querySelector<HTMLElement>('[data-tour="review-comment"] button')
    if (!target) return
    if (stableLeaf && target.hasAttribute('data-review-leaf')) target.tabIndex = -1
    document.getSelection()?.removeAllRanges()
    target.focus({ preventScroll: true })
    if (scroll) target.scrollIntoView({ block: 'center', behavior: 'instant' })
  }
  function continueReading() {
    const discard = !!selectedId && untouchedSuggestions.delete(selectedId) && !disabled
      && selected?.body === '' && selected.replacement === selected.quote
    // A discarded suggestion removes its highlight; focus its stable source leaf.
    locateSelection(selectedId, false, discard)
    if (discard) {
      update({ ...current, annotations: current.annotations.filter((note) => note.id !== selectedId) })
    }
    selectedId = null
  }
  function commentPreview(annotation: ReviewAnnotation) {
    const projection = fieldAttachmentText(annotation.body, $inputToolsState.attachments)
    return projection.text.replace(/\s+/g, ' ').trim() || tr(projection.attachmentIds.length ? 'Attachments'
      : annotation.kind === 'suggestion' ? (annotation.replacement === '' ? 'Suggest deletion' : 'Suggested wording') : 'Write your comment…')
  }
  function sourceLabel(annotation: ReviewAnnotation) { return annotation.start === null ? tr('Whole document') : tr('Selected passage') }
  async function revealVoiceTarget(sequence: number | undefined, target: SpeechTarget | null | undefined, review: DocumentReviewState) {
    if (sequence === undefined || sequence === revealedSequence || target?.requestId !== $voiceState.requestId || target?.destination.kind !== 'review_annotation') return
    const destination = target.destination
    if (destination.sourceVersion !== data.source_version) return
    const annotation = review.annotations.find((item) => item.id === destination.annotationId)
    if (!annotation) return
    revealedSequence = sequence
    await showComment(annotation)
    root?.querySelector<HTMLElement>(`[data-comment-id="${annotation.id}"] [data-review-field="${destination.field}"]`)?.focus({ preventScroll: true })
  }
</script>

<svelte:document onselectionchange={captureSelection} />
<section bind:this={root} bind:clientWidth={rootWidth} class="document-review min-h-0 min-w-0" aria-label={tr('Markdown document review')}>
  <header class="border-b pb-4">
    <div class="flex items-start gap-3">
      <span class="mt-0.5 rounded-lg border bg-muted/50 p-2 text-muted-foreground"><FileText class="size-5" /></span>
      <div class="min-w-0 flex-1">
        <h2 class="m-0 break-words text-lg font-semibold leading-7">{data.title}</h2>
        <p class="m-0 mt-1 flex flex-wrap items-center gap-3 text-xs text-muted-foreground"><span>{data.source_version}</span><span class="inline-flex items-center gap-1"><LockKeyhole class="size-3" />{tr('Original text is preserved')}</span></p>
      </div>
    </div>
    <div data-tour="review-comment" class="mt-3 flex flex-wrap items-center gap-3">
      <button type="button" {disabled} onclick={() => void addComment(whole, 'comment')} class="inline-flex items-center gap-1 rounded-md border px-3 py-2 text-xs disabled:opacity-40"><MessageSquare class="size-3.5" />{tr('Comment on document')}</button>
      <span class="text-xs text-muted-foreground">{tr('Select text, leave a note, and keep reading.')}</span>
    </div>
  </header>
  {#if selectedText && !disabled}
    <div class="pointer-events-none sticky top-0 z-10 h-0"><div data-review-selection-toolbar class="pointer-events-auto absolute inset-x-0 top-2 flex flex-wrap items-center gap-2 rounded-lg border border-primary/30 bg-background p-3 shadow-sm" aria-label={tr('Selected passage')}>
      <p class="m-0 min-w-0 flex-1 truncate text-xs text-muted-foreground" title={selectionPreview}>“{selectionPreview}”</p>
      <button type="button" onpointerdown={(event) => event.preventDefault()} onclick={() => selectedText && void addComment(selectedText, 'comment')} class="inline-flex items-center gap-1 rounded-md bg-primary px-3 py-2 text-xs text-primary-foreground"><MessageSquare class="size-3.5" />{tr('Comment')}</button>
      <button type="button" disabled={!rewriteAllowed} title={rewriteAllowed ? undefined : tr('Select a shorter passage to suggest wording (8,000 characters maximum).')} onpointerdown={(event) => event.preventDefault()} onclick={() => selectedText && void addComment(selectedText, 'suggestion')} class="inline-flex items-center gap-1 rounded-md border px-3 py-2 text-xs disabled:opacity-40"><PencilLine class="size-3.5" />{tr('Suggest change')}</button>
      <button type="button" aria-label={tr('Clear selection')} onclick={() => { selectedText = null; document.getSelection()?.removeAllRanges() }} class="rounded p-1"><X class="size-4" /></button>
    </div></div>
  {/if}
  {#if selectionMessage}<p role="status" class="my-3 text-xs text-amber-700 dark:text-amber-400">{selectionMessage}</p>{/if}
  <div id="review-original" role="region" aria-label={tr('Original')} class="py-4">
    {#if model.fallback}<p class="text-xs text-muted-foreground">{tr('Some content is shown as Markdown source to preserve exact selection anchors.')}</p>{/if}
    <div bind:this={sourceRoot} class="review-source-layout" style:min-height={selected && (rootWidth >= 900 || floatingCard) ? `${cardTop + cardHeight}px` : undefined}>
      {#each groups as group (group.index)}
        <div data-review-block={group.index}>
          {#if group.node}<div class="review-source-body"><ReviewMarkdown document={model} nodes={[group.node]} annotations={current.annotations} {selectedId} onSelect={selectHighlight} /></div>{/if}
          {#if group.notes.length}
            <div data-review-note-region class="review-notes">
              <div class="review-note-index flex flex-wrap gap-1.5" aria-label={tr('Comments')}>
                {#each group.notes as annotation (annotation.id)}
                  <button type="button" data-review-note-preview={annotation.id} aria-expanded={selectedId === annotation.id} onclick={() => void openComment(annotation)} title={commentPreview(annotation)}
                    class="inline-flex max-w-64 items-center gap-1.5 rounded-md border border-transparent bg-muted/40 px-2 py-1 text-left text-xs text-muted-foreground hover:bg-muted aria-expanded:border-primary/40 aria-expanded:text-primary">
                    <span class="font-medium">{current.annotations.indexOf(annotation) + 1}</span><MessageSquare class="size-3 shrink-0" /><span class="truncate">{commentPreview(annotation)}</span>
                  </button>
                {/each}
              </div>
              {#if selected && group.notes.some((note) => note.id === selectedId)}
                {#key selected.id}<div class="review-active-card" class:floating-card={floatingCard} data-review-card-placement={rootWidth >= 900 ? 'sidebar' : floatingCard ? 'floating' : 'inline'} bind:clientHeight={cardHeight} style:top={`${cardTop}px`}>
                  {#if selected.start !== null}<button type="button" onclick={() => locateSelection()} class="mb-2 text-xs text-primary underline">{tr('Locate in original')}</button>{/if}
                  <ReviewCommentCard bind:this={selectedComment} annotation={selected} sourceText={selected.quote ?? data.markdown} sourceLabel={sourceLabel(selected)} sourceVersion={data.source_version} {disabled} onUpdate={changeComment} onDelete={() => void deleteComment(selected!.id)} onCollapse={continueReading} />
                </div>{/key}
              {/if}
            </div>
          {/if}
        </div>
      {/each}
      {#if !selected}<p class="review-rail-hint text-xs leading-5 text-muted-foreground">{tr('Notes stay beside the text. Select a highlight or a numbered note to reopen it.')}</p>{/if}
    </div>
  </div>
  <footer class="mt-3 border-t pt-4">
    <fieldset data-tour="review-verdict" {disabled} class="m-0 border-0 p-0">
      <legend class="mb-2 text-xs font-medium">{tr('Review decision')}</legend>
      <div class="flex flex-wrap gap-2"><button type="button" {disabled} aria-pressed={current.verdict === 'ready'} onclick={() => update({ ...current, verdict: current.verdict === 'ready' ? null : 'ready' })} class="rounded-lg border px-3 py-2 text-xs aria-pressed:border-emerald-600 aria-pressed:bg-emerald-500/10 disabled:opacity-60">{tr('Ready as written')}</button><button type="button" {disabled} aria-pressed={current.verdict === 'changes_requested'} onclick={() => update({ ...current, verdict: current.verdict === 'changes_requested' ? null : 'changes_requested' })} class="rounded-lg border px-3 py-2 text-xs aria-pressed:border-amber-600 aria-pressed:bg-amber-500/10 disabled:opacity-60">{tr('Changes requested')}</button></div>
    </fieldset>
    <p class="mb-0 mt-3 text-xs leading-5 text-muted-foreground">{tr('Choose a decision, then submit with your overall feedback.')}</p>
    {#if current.verdict && issue}<p role="status" class="mt-2 text-xs text-amber-700 dark:text-amber-400">{tr(issue)}</p>{/if}
  </footer>
</section>

<style>
  .document-review { container-type: inline-size; }
  .review-source-layout { position: relative; }
  .review-source-body { min-width: 0; }
  .review-notes { margin: .5rem 0 1rem; }
  .review-active-card { margin-top: .75rem; min-width: 0; }
  .review-active-card.floating-card { position: absolute; left: 0; right: 0; z-index: 5; margin-top: 0; max-height: min(440px, 65vh); overflow: auto; overscroll-behavior: contain; padding: .5rem; border: 1px solid var(--border); border-radius: .75rem; background: var(--background); box-shadow: 0 8px 24px rgb(0 0 0 / .12); }
  .review-rail-hint { display: none; }
  @container (min-width: 900px) {
    .review-source-body, .review-note-index { width: calc(100% - 348px); }
    .review-active-card { position: absolute; right: 0; width: 320px; margin-top: 0; }
    .review-rail-hint { display: block; position: absolute; top: 1rem; right: 0; width: 300px; padding: 1rem; border-left: 1px solid var(--border); }
  }
</style>

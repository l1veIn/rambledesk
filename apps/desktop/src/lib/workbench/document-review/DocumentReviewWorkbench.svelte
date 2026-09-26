<script lang="ts">
  import { tick } from 'svelte'
  import { get } from 'svelte/store'
  import { FileText, MessageSquare, PencilLine, LockKeyhole, X, Mic, Trash2, RotateCcw, ChevronRight } from '@lucide/svelte'
  import type { DocumentReviewData, ReviewAnnotation } from '../../generated/feedback'
  import { locale } from '../../preferences'
  import { annotatedSegments, emptyReviewState, prepareParagraphAnnotation, toggleParagraphRemoval, validateReviewState, type DocumentReviewState, type ReviewAnchor } from './reviewModel'
  import { selectionAnchor } from './reviewSelection'
  import { reviewText } from './reviewI18n'
  import ReviewCommentCard from './ReviewCommentCard.svelte'
  import { reviewAnnotationVoiceTarget, unavailableVoiceInputState, useVoiceInput } from '../../speech/voiceInputContext'
  import type { SpeechTarget } from '../../speech/speechDraftQueue'
  import { fieldAttachmentText } from '../../input/fieldAttachmentText'
  import { unavailableInputToolsState, useInputTools } from '../../input/inputToolsContext'

  export let data: DocumentReviewData
  export let state: DocumentReviewState | null = null
  export let disabled = false
  export let onChange: (state: DocumentReviewState) => void
  const voice = useVoiceInput()
  const voiceState = voice?.state ?? unavailableVoiceInputState
  const inputToolsState = useInputTools()?.state ?? unavailableInputToolsState
  let root: HTMLElement
  let selectedId: string | null = null
  let selectedText: ReviewAnchor | null = null
  let selectionMessage = ''
  let revealedSequence: number | undefined
  $: current = state ?? emptyReviewState()
  $: removedCount = current.paragraph_marks.filter((item) => item.decision === 'remove').length
  $: issue = validateReviewState(data, current)
  $: void revealVoiceTarget($voiceState.revealSequence, $voiceState.revealTarget, current)
  const tr = (text: string) => reviewText($locale, text)
  function update(next: DocumentReviewState) { if (!disabled) { current = next; state = next; onChange(next) } }
  function captureSelection() {
    if (!root || disabled) return
    const anchor = selectionAnchor(root, document.getSelection())
    if (anchor === 'cross_paragraph') { selectedText = null; selectionMessage = tr('Select text within one paragraph. Use paragraph comments for broader feedback.') }
    else if (anchor) { selectedText = anchor; selectionMessage = '' }
    else if (!root.querySelector('[data-review-selection-toolbar]')?.contains(document.activeElement)) { selectedText = null; selectionMessage = '' }
  }
  async function addComment(anchor: ReviewAnchor, kind: ReviewAnnotation['kind'], speak = false) {
    if (disabled) return
    const prepared = prepareParagraphAnnotation(data, current, anchor, kind, () => `note-${crypto.randomUUID()}`, selectedId)
    if (!prepared) { selectionMessage = tr('Comment limit reached.'); return }
    const annotation = prepared.annotation
    selectedText = null
    selectionMessage = ''
    document.getSelection()?.removeAllRanges()
    if (prepared.state !== current) update(prepared.state)
    await openComment(annotation)
    if (speak && voice && !disabled && !get(voiceState).disabled) {
      const target = reviewAnnotationVoiceTarget(get(voiceState), annotation.id, 'body', data.source_version, paragraphLabel(annotation.paragraph_id))
      if (target) void voice.start(target)
    }
  }
  function changeComment(annotation: ReviewAnnotation) { update({ ...current, annotations: current.annotations.map((item) => item.id === annotation.id ? annotation : item) }) }
  async function deleteComment(id: string) {
    if (disabled) return
    const annotation = current.annotations.find((item) => item.id === id)
    if (!annotation) return
    update({ ...current, annotations: current.annotations.filter((item) => item.id !== id) })
    if (selectedId === id) selectedId = null
    await tick()
    root?.querySelector<HTMLElement>(`[data-paragraph-id="${annotation.paragraph_id}"] [data-tour="review-comment"] button`)?.focus({ preventScroll: true })
  }
  async function showInlineComment(annotation: ReviewAnnotation) {
    selectedId = annotation.id
    await tick()
    root?.querySelector<HTMLElement>(`[data-paragraph-id="${annotation.paragraph_id}"]`)?.scrollIntoView({ block: 'nearest', behavior: 'smooth' })
  }
  async function openComment(annotation: ReviewAnnotation) {
    await showInlineComment(annotation)
    if (disabled) return
    const input = root?.querySelector<HTMLElement>(`[data-comment-id="${annotation.id}"] [data-review-field="body"]`)
    if (!input) return
    input.focus({ preventScroll: true })
    // Continue the existing note at its end, without selecting or replacing its text.
    const range = document.createRange()
    range.selectNodeContents(input); range.collapse(false)
    const selection = document.getSelection()
    selection?.removeAllRanges(); selection?.addRange(range)
  }
  function commentPreview(annotation: ReviewAnnotation) {
    const projection = fieldAttachmentText(annotation.body, $inputToolsState.attachments)
    return projection.text.replace(/\s+/g, ' ').trim()
      || tr(projection.attachmentIds.length ? 'Attachments' : 'Write your comment…')
  }
  function paragraphLabel(id: string) {
    const index = data.paragraphs.findIndex((paragraph) => paragraph.id === id)
    return data.paragraphs[index]?.label ?? `${tr('Paragraph')} ${index + 1}`
  }
  async function revealVoiceTarget(sequence: number | undefined, target: SpeechTarget | null | undefined, review: DocumentReviewState) {
    if (sequence === undefined || sequence === revealedSequence || target?.requestId !== $voiceState.requestId || target?.destination.kind !== 'review_annotation') return
    const destination = target.destination
    if (destination.sourceVersion !== data.source_version) return
    const annotation = review.annotations.find((item) => item.id === destination.annotationId)
    if (!annotation) return
    revealedSequence = sequence
    await showInlineComment(annotation)
    root?.querySelector<HTMLElement>(`[data-comment-id="${annotation.id}"] [data-review-field="${destination.field}"]`)?.focus({ preventScroll: true })
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
        <p class="m-0 mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
          <span>{data.source_version}</span>
          <span class="inline-flex items-center gap-1"><LockKeyhole class="size-3" />{tr('Original text is preserved')}</span>
          {#if removedCount > 0}<span aria-label={tr('Review progress')}>{tr('Paragraphs deleted')} {removedCount}</span>{/if}
        </p>
      </div>
    </div>
  </header>

  {#if selectedText && !disabled}
    <div class="pointer-events-none sticky top-0 z-10 h-0">
    <div data-review-selection-toolbar class="pointer-events-auto absolute inset-x-0 top-2 flex flex-wrap items-center gap-2 rounded-lg border border-primary/30 bg-background p-3 shadow-sm" aria-label={tr('Selected passage')}>
      <p class="m-0 min-w-0 flex-1 truncate text-xs text-muted-foreground">“{selectedText.quote}”</p>
      <button type="button" onpointerdown={(event) => event.preventDefault()} onclick={() => selectedText && void addComment(selectedText, 'comment')} class="inline-flex items-center gap-1 rounded-md bg-primary px-3 py-2 text-xs text-primary-foreground"><MessageSquare class="size-3.5" />{tr('Comment')}</button>
      {#if voice}<button type="button" disabled={$voiceState.disabled} onpointerdown={(event) => event.preventDefault()} onclick={() => selectedText && void addComment(selectedText, 'comment', true)} class="inline-flex items-center gap-1 rounded-md border px-3 py-2 text-xs disabled:opacity-40"><Mic class="size-3.5" />{tr('Speak comment')}</button>{/if}
      <button type="button" onpointerdown={(event) => event.preventDefault()} onclick={() => selectedText && void addComment(selectedText, 'suggestion')} class="inline-flex items-center gap-1 rounded-md border px-3 py-2 text-xs"><PencilLine class="size-3.5" />{tr('Suggest rewrite')}</button>
      <button type="button" aria-label={tr('Clear selection')} onclick={() => { selectedText = null; document.getSelection()?.removeAllRanges() }} class="rounded p-1"><X class="size-4" /></button>
    </div>
    </div>
  {/if}
  {#if selectionMessage}<p role="status" class="my-3 text-xs text-amber-700 dark:text-amber-400">{selectionMessage}</p>{/if}

  <div id="review-original" role="region" aria-label={tr('Original')} class="py-4">
    <p class="mb-5 text-xs leading-5 text-muted-foreground">{tr('Select text to comment or suggest a rewrite.')} {tr('One comment per paragraph. Reopen it to add more feedback.')}</p>
    <div class="grid gap-6">
      {#each data.paragraphs as paragraph, index (paragraph.id)}
        {@const annotations = current.annotations.filter((item) => item.paragraph_id === paragraph.id)}
        {@const selected = annotations.find((item) => item.id === selectedId)}
        {@const mark = current.paragraph_marks.find((item) => item.paragraph_id === paragraph.id)}
        {@const removed = mark?.decision === 'remove'}
        <article data-paragraph-id={paragraph.id} class="review-paragraph min-w-0 rounded-lg border-l-2 pl-4" class:border-rose-500={removed} class:border-transparent={!removed}>
          <div class="mb-2 flex flex-wrap items-center gap-2 text-xs">
            <span class="font-mono text-[10px] tabular-nums text-muted-foreground">{String(index + 1).padStart(2, '0')}</span>
            {#if paragraph.label}<span class="min-w-0 flex-1 truncate font-medium text-muted-foreground">{paragraph.label}</span>{/if}
            <button type="button" data-tour="review-delete" {disabled} aria-label={`${tr(removed ? 'Restore paragraph' : 'Delete paragraph')} ${index + 1}`} title={tr(removed ? 'Restore paragraph' : 'Delete paragraph')} aria-pressed={removed}
              onclick={() => update(toggleParagraphRemoval(current, paragraph.id))}
              class="ml-auto inline-flex items-center gap-1 rounded-md px-2 py-1.5 text-muted-foreground hover:bg-muted hover:text-destructive aria-pressed:text-destructive disabled:opacity-40">
              {#if removed}<RotateCcw class="size-3.5" />{tr('Restore paragraph')}{:else}<Trash2 class="size-3.5" />{tr('Delete paragraph')}{/if}
            </button>
            <span class="inline-flex items-center gap-2" data-tour="review-comment">
            <button type="button" {disabled} aria-label={`${tr('Comment on paragraph')} ${index + 1}`} title={tr(annotations.length ? 'Continue comment' : 'Paragraph comment')} onclick={() => void addComment({ paragraph_id: paragraph.id, start: null, end: null, quote: null }, 'comment')} class="rounded-md p-1.5 text-muted-foreground hover:bg-muted disabled:opacity-40"><MessageSquare class="size-3.5" /></button>
            {#if voice}<button type="button" disabled={disabled || $voiceState.disabled} aria-label={`${tr('Speak comment on paragraph')} ${index + 1}`} title={tr('Speak comment')} onclick={() => void addComment({ paragraph_id: paragraph.id, start: null, end: null, quote: null }, 'comment', true)} class="rounded-md p-1.5 text-muted-foreground hover:bg-muted disabled:opacity-40"><Mic class="size-3.5" /></button>{/if}
            </span>
          </div>
          <div role="group" aria-label={paragraphLabel(paragraph.id)}>
            <p data-review-text={paragraph.id} data-paragraph-deleted={removed ? 'true' : undefined} class="review-source m-0 whitespace-pre-wrap break-words text-[15px] leading-8 decoration-destructive/70 decoration-[1.5px]" class:line-through={removed} class:text-muted-foreground={removed}>{#each annotatedSegments(paragraph.text, annotations) as segment}{#if segment.annotationIds.length}<span role="button" tabindex="0" data-annotation-ids={segment.annotationIds.join(' ')} onclick={activateHighlight} onkeydown={(event) => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); selectedId = segment.annotationIds[0] } }} class={`cursor-pointer rounded-sm border-b-2 text-inherit focus-visible:outline-ring ${segment.annotationIds.includes(selectedId ?? '') ? 'border-primary bg-primary/20' : 'border-amber-400 bg-amber-300/20'}`}>{segment.text}</span>{:else}{segment.text}{/if}{/each}</p>
          </div>
          {#if annotations.length}
            <div class="mt-2 grid gap-1.5">
              {#each annotations as annotation (annotation.id)}
                {#if selectedId !== annotation.id}
                  <button type="button" data-review-note-preview={annotation.id} aria-expanded="false" onclick={() => void openComment(annotation)} title={commentPreview(annotation)}
                    class="flex min-w-0 items-center gap-2 rounded-lg border bg-muted/20 px-3 py-2 text-left text-xs text-muted-foreground hover:border-primary/40 hover:bg-muted/50">
                    <MessageSquare class="size-3.5 shrink-0 text-primary" />
                    <span class="min-w-0 flex-1 truncate">{commentPreview(annotation)}</span>
                    <ChevronRight class="size-3.5 shrink-0" />
                  </button>
                {/if}
              {/each}
            </div>
          {/if}
          {#if selected}
            {#key selected.id}
            <div class="mt-3"><ReviewCommentCard annotation={selected} paragraphLabel={paragraphLabel(paragraph.id)} sourceVersion={data.source_version} {disabled} onUpdate={changeComment} onDelete={() => void deleteComment(selected.id)} onCollapse={() => selectedId = null} /></div>
            {/key}
          {/if}
        </article>
      {/each}
    </div>
  </div>

  <footer class="mt-3 border-t pt-4">
    <fieldset data-tour="review-verdict" {disabled} class="m-0 border-0 p-0">
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

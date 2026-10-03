<script lang="ts">
  import { tick } from 'svelte'
  import { Headphones, Film, Maximize2, MessageSquare, Trash2, X } from '@lucide/svelte'
  import type { MediaReviewComment, MediaReviewData } from '../../generated/feedback'
  import type { MediaReviewState } from '../definitions/media_review/state'
  import type { InputTarget } from '../../domain/inputTarget'
  import type { MediaReviewController } from './mediaController'
  import MediaPlayer from './MediaPlayer.svelte'
  import WorkbenchTextField from '../../input/WorkbenchTextField.svelte'
  import { fieldAttachmentText } from '../../input/fieldAttachmentText'
  import { locale } from '../../preferences'
  import { unavailableInputToolsState, useInputTools } from '../../input/inputToolsContext'
  import { unavailableVoiceInputState, useVoiceInput, workbenchFieldVoiceTarget } from '../../speech/voiceInputContext'
  import { emptyMediaReviewState, mediaAnchorLabel, mediaTime, prepareMediaComment, sortedMediaComments, validateMediaReviewState, type MediaCommentSort } from './mediaModel'
  import { mediaReviewText } from './mediaI18n'
  export let data: MediaReviewData
  export let state: MediaReviewState | null = null
  export let requestId: string
  export let runtime: MediaReviewController | undefined = undefined
  export let disabled = false, readOnly = false
  export let onChange: (state: MediaReviewState) => void
  export let onOpenExpanded: (() => void) | undefined = undefined
  const voiceState = useVoiceInput()?.state ?? unavailableVoiceInputState
  const toolsState = useInputTools()?.state ?? unavailableInputToolsState
  const tr = (source: string) => mediaReviewText($locale, source)
  let player: MediaPlayer, field: WorkbenchTextField | undefined, root: HTMLElement
  let selectedId: string | null = null, position = 0, ready = false, sourceKey = ''
  let rangeStart: number | null = null, rangeEnd: number | null = null, message = ''
  let order: MediaCommentSort = 'time', revealedSequence: number | undefined
  $: current = state ?? emptyMediaReviewState()
  $: editable = !disabled && !readOnly
  $: source = `${requestId}:${JSON.stringify(data)}`
  $: if (sourceKey !== source) { sourceKey = source; selectedId = null; rangeStart = null; rangeEnd = null; position = 0; ready = false; message = '' }
  $: comments = sortedMediaComments(current.comments, order)
  $: selected = current.comments.find((comment) => comment.id === selectedId)
  $: target = selected ? workbenchFieldVoiceTarget($voiceState, { workbenchType: 'media_review', version: 1, field: 'comment_body',
    entityId: selected.id, sourceVersion: data.source_version, label: `${data.media_file_name} · ${mediaAnchorLabel(selected)}` }) : null
  $: issue = validateMediaReviewState(data, current)
  $: void revealField($voiceState.revealSequence, $voiceState.revealTarget, current)
  const cursor = (ms: number) => Math.max(0, Math.min(data.duration_ms, Math.round(ms)))
  function update(next: MediaReviewState) { if (editable) { current = next; state = next; onChange(next) } }
  function seek(ms: number) { position = cursor(ms); player?.seek(position) }
  async function openComment(comment: MediaReviewComment, focus = true) {
    player?.pause(); seek(comment.start_ms); selectedId = comment.id
    rangeStart = comment.end_ms === null ? null : comment.start_ms; rangeEnd = comment.end_ms
    await tick()
    if (focus && editable && selectedId === comment.id) field?.focusAtEnd()
  }
  async function addComment(start_ms = position, end_ms: number | null = null) {
    if (!editable || !ready) return
    player?.pause()
    const prepared = prepareMediaComment(data, current, { start_ms: cursor(start_ms), end_ms }, () => `comment-${crypto.randomUUID()}`)
    if (!prepared) { message = tr('Comment limit reached.'); return }
    if (prepared.state !== current) update(prepared.state)
    await openComment(prepared.comment)
  }
  function markIn() { if (editable && ready) { player?.pause(); rangeStart = position; rangeEnd = null; message = '' } }
  function markOut() {
    if (!editable || !ready) return
    player?.pause()
    if (rangeStart === null || position <= rangeStart) { message = tr('Choose an end after the start.'); return }
    rangeEnd = position; message = ''
  }
  async function remove(id: string) {
    if (!editable) return
    update({ ...current, comments: current.comments.filter((comment) => comment.id !== id) }); selectedId = null
    await tick(); root.querySelector<HTMLButtonElement>('[data-media-add-comment]')?.focus()
  }
  function body(id: string, value: string) {
    update({ ...current, comments: current.comments.map((comment) => comment.id === id ? { ...comment, body: value } : comment) })
  }
  function preview(comment: MediaReviewComment) {
    const projected = fieldAttachmentText(comment.body, $toolsState.attachments)
    return projected.text.replace(/\s+/g, ' ').trim() || tr(projected.attachmentIds.length ? 'Attachments' : 'Write your comment…')
  }
  async function revealField(sequence: number | undefined, input: InputTarget | null | undefined, review: MediaReviewState) {
    const destination = input?.destination
    if (sequence === undefined || sequence === revealedSequence || input?.requestId !== requestId || destination?.kind !== 'workbench_field'
      || destination.workbenchType !== 'media_review' || destination.version !== 1 || destination.field !== 'comment_body'
      || destination.sourceVersion !== data.source_version) return
    const comment = review.comments.find((item) => item.id === destination.entityId)
    if (comment) { revealedSequence = sequence; await openComment(comment) }
  }
</script>

<section bind:this={root} class="@container flex min-w-0 flex-col gap-4 p-4" aria-label={tr('Media review')} data-media-review>
  <header class="flex items-start gap-3">
    {#if data.media_kind === 'video'}<Film class="mt-1 size-5 shrink-0 text-muted-foreground" />{:else}<Headphones class="mt-1 size-5 shrink-0 text-muted-foreground" />{/if}
    <div class="min-w-0 flex-1"><h2 class="m-0 break-words text-base font-semibold">{data.title}</h2><p class="m-0 mt-1 break-all text-xs text-muted-foreground">{data.media_file_name} · {data.source_version}</p></div>
    {#if onOpenExpanded}<button type="button" class="inline-flex shrink-0 items-center gap-1.5 rounded-md border px-3 py-2 text-xs" onclick={() => { player?.pause(); onOpenExpanded?.() }}><Maximize2 class="size-3.5" />{tr('Full screen review')}</button>{/if}
  </header>
  <div class="grid min-w-0 gap-4 @3xl:grid-cols-[minmax(0,1fr)_19rem]">
    <div class="min-w-0">
      <div data-tour="media-player">{#key source}<MediaPlayer bind:this={player} {data} {runtime} onClock={(ms) => position = cursor(ms)} onReady={(value) => ready = value} />{/key}</div>
      <p class="mb-2 mt-3 text-[11px] leading-5 text-muted-foreground">{tr('Playback starts only when you choose to play. Comments pause playback.')}</p>
      <div class="rounded-lg border p-3" data-tour="media-timeline">
        <div class="mb-3 flex items-center justify-between gap-3 font-mono text-xs"><time>{mediaTime(position)}</time><time>{mediaTime(data.duration_ms)}</time></div>
        <div class="relative mb-1 h-5" aria-label={tr('Comments')}>
          {#if rangeStart !== null && rangeEnd !== null}<div class="absolute top-1 h-2 rounded bg-primary/20" style:left={`${rangeStart / data.duration_ms * 100}%`} style:width={`${(rangeEnd - rangeStart) / data.duration_ms * 100}%`}></div>{/if}
          {#each current.comments as comment (comment.id)}
            <button type="button" data-media-marker={comment.id} aria-label={`${tr('Comment')} ${mediaAnchorLabel(comment)}`} title={mediaAnchorLabel(comment)}
              onclick={() => void openComment(comment)} class="absolute top-0 h-4 min-w-2 rounded border border-primary bg-primary/60 focus-visible:ring-2 focus-visible:ring-ring"
              style:left={`${Math.min(99, comment.start_ms / data.duration_ms * 100)}%`} style:width={`${Math.max(0.5, ((comment.end_ms ?? comment.start_ms) - comment.start_ms) / data.duration_ms * 100)}%`}></button>
          {/each}
        </div>
        <input type="range" aria-label={tr('Seek media')} min="0" max={data.duration_ms} step="1" value={position} disabled={!ready} class="w-full accent-primary"
          oninput={(event) => seek(Number(event.currentTarget.value))}
          onkeydown={(event) => { if (!event.ctrlKey && !event.metaKey && !event.altKey && (event.key.toLowerCase() === 'i' || event.key.toLowerCase() === 'o')) { event.preventDefault(); event.key.toLowerCase() === 'i' ? markIn() : markOut() } }} />
      </div>
      <div class="mt-3 flex flex-wrap items-center gap-2" data-tour="media-selection">
        {#if editable}
          <button type="button" data-media-add-comment disabled={!ready || current.comments.length >= 500} onclick={() => void addComment()} class="inline-flex items-center gap-1.5 rounded-md bg-primary px-3 py-2 text-xs text-primary-foreground disabled:opacity-40"><MessageSquare class="size-3.5" />{tr('Comment at current time')}</button>
          <button type="button" disabled={!ready} onclick={markIn} class="rounded-md border px-3 py-2 text-xs disabled:opacity-40">{tr('Mark in')}</button>
          <button type="button" disabled={!ready || rangeStart === null} onclick={markOut} class="rounded-md border px-3 py-2 text-xs disabled:opacity-40">{tr('Mark out')}</button>
        {/if}
        {#if rangeStart !== null}<span class="font-mono text-xs">{mediaTime(rangeStart)}{rangeEnd !== null ? ` – ${mediaTime(rangeEnd)}` : ' – …'}</span>
          <button type="button" aria-label={tr('Clear range')} title={tr('Clear range')} onclick={() => { rangeStart = null; rangeEnd = null }} class="rounded-md p-1.5 hover:bg-muted"><X class="size-3.5" /></button>{/if}
        {#if editable && rangeStart !== null && rangeEnd !== null}
          <div class="grid w-full gap-2 rounded-md bg-muted/20 p-3">
            <label class="text-xs">{tr('Range start')}<input type="range" min="0" max={rangeEnd - 1} step="1" value={rangeStart} class="mt-1 w-full accent-primary" oninput={(event) => { rangeStart = Number(event.currentTarget.value); seek(rangeStart) }} /></label>
            <label class="text-xs">{tr('Range end')}<input type="range" min={rangeStart + 1} max={data.duration_ms} step="1" value={rangeEnd} class="mt-1 w-full accent-primary" oninput={(event) => { rangeEnd = Number(event.currentTarget.value); seek(rangeEnd) }} /></label>
            <button type="button" disabled={!ready || current.comments.length >= 500} onclick={() => void addComment(rangeStart!, rangeEnd)} class="justify-self-start rounded-md border px-3 py-2 text-xs disabled:opacity-40">{tr('Add range comment')}</button>
          </div>
        {/if}
      </div>
    </div>
    <aside class="min-w-0 rounded-lg border bg-muted/10 p-3" aria-label={tr('Comments')} data-tour="media-comments">
      <div class="mb-3 flex flex-wrap items-center gap-3"><h3 class="m-0 flex-1 text-xs font-semibold">{tr('Comments')} · {current.comments.length}</h3>
        <select bind:value={order} aria-label={tr('Sort comments')} class="max-w-full rounded-md border bg-background px-2 py-1.5 text-xs">
          <option value="time">{tr('By time')}</option><option value="newest">{tr('Newest first')}</option><option value="oldest">{tr('Oldest first')}</option>
        </select></div>
      {#if !comments.length}<p class="m-0 text-xs leading-5 text-muted-foreground">{tr('No comments yet. You can also submit only overall feedback.')}</p>{/if}
      <div class="grid max-h-[45vh] gap-2 overflow-y-auto">
        {#each comments as comment (comment.id)}
          <button type="button" data-media-comment={comment.id} aria-expanded={selectedId === comment.id} onclick={() => void openComment(comment)}
            class="grid min-w-0 gap-1 rounded-md border bg-background p-3 text-left text-xs hover:border-primary/50 aria-expanded:border-primary aria-expanded:bg-primary/5">
            <time class="font-mono text-[11px] text-primary">{mediaAnchorLabel(comment)}</time><span class="truncate">{preview(comment)}</span>
          </button>
        {/each}
      </div>
      {#if selected}{#key selected.id}
        <div class="mt-3 grid gap-3 rounded-md border bg-background p-3" data-media-comment-editor>
          <div class="flex min-w-0 items-center gap-2"><time class="min-w-0 flex-1 break-words font-mono text-[11px]">{mediaAnchorLabel(selected)}</time>
            {#if editable}<button type="button" aria-label={tr('Delete comment')} title={tr('Delete comment')} onclick={() => void remove(selected!.id)} class="rounded-md p-1.5 hover:bg-destructive/10"><Trash2 class="size-3.5" /></button>{/if}
            <button type="button" aria-label={tr('Close comment')} title={tr('Close comment')} onclick={() => selectedId = null} class="rounded-md p-1.5 hover:bg-muted"><X class="size-3.5" /></button>
          </div>
          <WorkbenchTextField bind:this={field} value={selected.body} {target} maxLength={4000} disabled={!editable} label={tr('Comment')} voiceLabel={tr('Speak comment')} onChange={(value) => body(selected!.id, value)} />
        </div>
      {/key}{/if}
    </aside>
  </div>
  {#if message || issue}<p class="m-0 text-xs text-amber-700 dark:text-amber-400" role="status">{message || tr(issue!)}</p>{/if}
</section>

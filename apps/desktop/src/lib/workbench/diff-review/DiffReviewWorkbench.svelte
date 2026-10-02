<script lang="ts">
  import { tick } from 'svelte'
  import { FileDiff, Maximize2, MessageSquare, Trash2, X } from '@lucide/svelte'
  import type { DiffReviewAnchor, DiffReviewComment, DiffReviewData } from '../../generated/feedback'
  import type { DiffReviewState } from '../definitions/diff_review/state'
  import type { InputTarget } from '../../domain/inputTarget'
  import { locale } from '../../preferences'
  import WorkbenchTextField from '../../input/WorkbenchTextField.svelte'
  import { fieldAttachmentText } from '../../input/fieldAttachmentText'
  import { unavailableInputToolsState, useInputTools } from '../../input/inputToolsContext'
  import { unavailableVoiceInputState, useVoiceInput, workbenchFieldVoiceTarget } from '../../speech/voiceInputContext'
  import { countUnifiedDiffLineChanges } from '../../agents/chat/line-change-stats'
  import { anchorIncludesLine, diffAnchorLabel, diffAnchorOffset, diffFileLabel, diffFileMetadata, diffWindow, parseReviewDiff, type ReviewDiffHunk } from './diffModel'
  import { emptyDiffReviewState, prepareDiffReviewComment, validateDiffReviewState } from './reviewModel'
  import { diffReviewText } from './diffReviewI18n'

  export let data: DiffReviewData
  export let state: DiffReviewState | null = null
  export let requestId: string
  export let disabled = false
  export let readOnly = false
  export let onChange: (state: DiffReviewState) => void
  export let onOpenExpanded: (() => void) | undefined = undefined
  const voiceState = useVoiceInput()?.state ?? unavailableVoiceInputState
  const toolsState = useInputTools()?.state ?? unavailableInputToolsState
  const pageSize = 1000
  let root: HTMLElement
  let commentField: WorkbenchTextField | undefined
  let selectedFileId = '', selectedId: string | null = null
  let selection: DiffReviewAnchor | null = null, selectionOrigin: number | null = null
  let pageStart = 0, sourceKey = '', revealedSequence: number | undefined
  let message = ''
  $: current = state ?? emptyDiffReviewState()
  $: editable = !disabled && !readOnly
  $: files = data.files.map((file) => ({ file, hunks: parseReviewDiff(file.diff) ?? [], stats: countUnifiedDiffLineChanges(file.diff) }))
  $: resetSource(`${requestId}:${JSON.stringify(data)}`)
  $: active = files.find(({ file }) => file.id === selectedFileId) ?? files[0]
  $: totalLines = active?.hunks.reduce((sum, hunk) => sum + hunk.lines.length, 0) ?? 0
  $: visibleHunks = diffWindow(active?.hunks ?? [], pageStart, pageSize)
  $: metadata = active ? diffFileMetadata(active.file.diff) : []
  $: selected = current.comments.find((comment) => comment.id === selectedId)
  $: target = selected ? workbenchFieldVoiceTarget($voiceState, { workbenchType: 'diff_review', version: 1,
    field: 'comment_body', entityId: selected.id, sourceVersion: data.source_version,
    label: diffAnchorLabel(data.files.find((file) => file.id === selected.anchor.file_id), selected.anchor) }) : null
  $: issue = validateDiffReviewState(data, current)
  $: void revealField($voiceState.revealSequence, $voiceState.revealTarget, current)
  const tr = (source: string, values: Record<string, string | number> = {}) => diffReviewText($locale, source, values)

  function resetSource(next: string) {
    if (sourceKey === next) return
    sourceKey = next; selectedFileId = data.files[0]?.id ?? ''; selectedId = null
    selection = null; selectionOrigin = null; pageStart = 0; message = ''
  }
  function update(next: DiffReviewState) { if (editable) { current = next; state = next; onChange(next) } }
  function selectFile(id: string) {
    selectedFileId = id; selection = null; selectionOrigin = null; pageStart = 0; selectedId = null; message = ''
  }
  function selectLine(hunk: number, side: 'old' | 'new', line: number, extend = false) {
    const same = selection?.file_id === active.file.id && selection.hunk_index === hunk && selection.side === side
    if (!extend || !same || selectionOrigin === null) selectionOrigin = line
    selection = { file_id: active.file.id, hunk_index: hunk, side,
      start_line: Math.min(selectionOrigin, line), end_line: Math.max(selectionOrigin, line) }
    message = ''
  }
  async function openComment(comment: DiffReviewComment, focus = true) {
    const file = files.find((item) => item.file.id === comment.anchor.file_id)
    if (file) { selectedFileId = file.file.id; pageStart = Math.floor(diffAnchorOffset(file.hunks, comment.anchor) / pageSize) * pageSize }
    selectedId = comment.id; selection = { ...comment.anchor }; selectionOrigin = comment.anchor.start_line
    await tick()
    root?.querySelector('[data-diff-comment-editor]')?.scrollIntoView({ block: 'nearest' })
    if (focus && editable && selectedId === comment.id) commentField?.focusAtEnd()
  }
  async function addComment(anchor = selection) {
    if (!editable || !anchor) return
    const prepared = prepareDiffReviewComment(data, current, anchor, () => `comment-${crypto.randomUUID()}`)
    if (!prepared) { message = tr('Comment limit reached.'); return }
    if (prepared.state !== current) update(prepared.state)
    await openComment(prepared.comment)
  }
  async function commentHunk(hunk: ReviewDiffHunk) {
    await addComment({ file_id: active.file.id, hunk_index: hunk.index, side: hunk.new.count ? 'new' : 'old', start_line: null, end_line: null })
  }
  function changeComment(id: string, body: string) {
    if (current.comments.some((comment) => comment.id === id)) update({ ...current, comments: current.comments.map((comment) => comment.id === id ? { ...comment, body } : comment) })
  }
  async function deleteComment(id: string) {
    if (!editable) return
    update({ ...current, comments: current.comments.filter((comment) => comment.id !== id) })
    selectedId = null
    await tick()
    root?.querySelector<HTMLButtonElement>('[data-diff-add-comment]')?.focus()
  }
  async function lineKey(event: KeyboardEvent, hunk: ReviewDiffHunk, side: 'old' | 'new', line: number) {
    if (event.ctrlKey || event.metaKey || event.altKey) return
    if (event.key.toLowerCase() === 'c') {
      event.preventDefault()
      if (!selection || selection.hunk_index !== hunk.index || selection.side !== side) selectLine(hunk.index, side, line)
      await addComment(); return
    }
    const rows = hunk.lines.flatMap((row) => row[side === 'old' ? 'oldLine' : 'newLine'] === null ? [] : [row[side === 'old' ? 'oldLine' : 'newLine']!])
    const index = rows.indexOf(line)
    const next = event.key === 'ArrowUp' ? rows[index - 1] : event.key === 'ArrowDown' ? rows[index + 1]
      : event.key === 'Home' ? rows[0] : event.key === 'End' ? rows.at(-1) : undefined
    if (next === undefined) return
    event.preventDefault()
    if (event.shiftKey && (!selection || selection.start_line === null || selection.hunk_index !== hunk.index || selection.side !== side)) selectLine(hunk.index, side, line)
    selectLine(hunk.index, side, next, event.shiftKey)
    // A range starts at its first line, but the caret must follow its moving end.
    pageStart = Math.floor(diffAnchorOffset(active.hunks, { ...selection!, start_line: next, end_line: next }) / pageSize) * pageSize
    await tick()
    root?.querySelector<HTMLButtonElement>(`[data-diff-hunk="${hunk.index}"][data-diff-side="${side}"][data-diff-line="${next}"]`)?.focus()
  }
  async function revealField(sequence: number | undefined, input: InputTarget | null | undefined, review: DiffReviewState) {
    const destination = input?.destination
    if (sequence === undefined || sequence === revealedSequence || input?.requestId !== requestId || destination?.kind !== 'workbench_field'
      || destination.workbenchType !== 'diff_review' || destination.version !== 1 || destination.field !== 'comment_body'
      || destination.sourceVersion !== data.source_version) return
    const comment = review.comments.find((item) => item.id === destination.entityId)
    if (!comment) return
    revealedSequence = sequence
    await openComment(comment)
  }
  function preview(comment: DiffReviewComment) {
    const body = fieldAttachmentText(comment.body, $toolsState.attachments)
    return body.text.replace(/\s+/g, ' ').trim() || tr(body.attachmentIds.length ? 'Attachments' : 'Write your comment…')
  }
</script>

<section bind:this={root} class="@container flex min-h-0 min-w-0 flex-col gap-4 p-4" aria-label={tr('Diff review')} data-diff-review>
  <header class="flex items-start gap-3">
    <FileDiff class="mt-1 size-5 shrink-0 text-muted-foreground" />
    <div class="min-w-0 flex-1"><h2 class="m-0 break-words text-base font-semibold">{data.title}</h2><p class="m-0 mt-1 break-all text-xs text-muted-foreground">{data.source_version}</p></div>
    {#if onOpenExpanded}<button type="button" class="inline-flex shrink-0 items-center gap-1.5 rounded-md border px-3 py-2 text-xs" onclick={onOpenExpanded}><Maximize2 class="size-3.5" />{tr('Full screen review')}</button>{/if}
  </header>
  <p class="m-0 text-xs leading-5 text-muted-foreground">{tr('The original diff is preserved. Add comments; write overall feedback on the right.')}</p>
  <nav class="flex flex-wrap gap-2" aria-label={tr('Files')} data-tour="diff-files">
    {#each files as item (item.file.id)}
      <button type="button" data-diff-file={item.file.id} aria-current={active.file.id === item.file.id ? 'page' : undefined}
        title={`${item.file.old_path} → ${item.file.new_path}`} onclick={() => selectFile(item.file.id)}
        class="flex min-w-0 max-w-full items-center gap-2 rounded-md border px-3 py-2 text-left text-xs aria-current:border-primary aria-current:bg-primary/10">
        <span class="truncate font-mono">{diffFileLabel(item.file)}</span><span class="shrink-0 text-emerald-700 dark:text-emerald-400">+{item.stats.additions}</span><span class="shrink-0 text-rose-700 dark:text-rose-400">−{item.stats.deletions}</span>
        {#if current.comments.some((comment) => comment.anchor.file_id === item.file.id)}<span class="shrink-0 text-muted-foreground">· {current.comments.filter((comment) => comment.anchor.file_id === item.file.id).length}</span>{/if}
      </button>
    {/each}
  </nav>
  <div class="grid min-w-0 gap-4 @3xl:grid-cols-[minmax(0,1fr)_20rem]">
    <div class="min-w-0">
      {#if active.file.old_path !== active.file.new_path}<p class="mb-2 mt-0 break-all font-mono text-[11px] text-muted-foreground">{active.file.old_path} → {active.file.new_path}</p>{/if}
      {#if editable}<p class="mb-2 mt-0 text-[11px] leading-5 text-muted-foreground">{tr('Choose an old or new line number. Shift-click or Shift + ↑/↓ selects a range within one hunk and side. Press C to comment.')}</p>{/if}
      <div class="mb-2 flex min-h-9 flex-wrap items-center gap-2" data-tour="diff-selection">
        {#if selection && selection.file_id === active.file.id}<span class="min-w-0 flex-1 truncate font-mono text-[11px]" title={diffAnchorLabel(active.file, selection)}>{diffAnchorLabel(active.file, selection)}</span>
          <button type="button" aria-label={tr('Clear selection')} title={tr('Clear selection')} onclick={() => { selection = null; selectionOrigin = null }} class="rounded-md p-1.5 hover:bg-muted"><X class="size-3.5" /></button>{/if}
        {#if editable}<button type="button" data-diff-add-comment disabled={!selection || current.comments.length >= 500} onclick={() => void addComment()}
          class="ml-auto inline-flex items-center gap-1.5 rounded-md bg-primary px-3 py-2 text-xs text-primary-foreground disabled:opacity-40"><MessageSquare class="size-3.5" />{tr('Add comment')}</button>{/if}
      </div>
      <!-- svelte-ignore a11y_no_noninteractive_tabindex (Keyboard users can scroll immutable source.) -->
      <div class="max-h-[65vh] overflow-auto rounded-md border font-mono text-[11px] leading-5" role="region" aria-label={diffFileLabel(active.file)} tabindex="0" data-diff-source>
        <div class="min-w-max">
          {#each metadata as line}<div class="whitespace-pre bg-muted/30 px-2 text-muted-foreground" data-diff-metadata>{line.text}</div>{/each}
          {#each visibleHunks as shown (shown.index)}
            {@const hunk = active.hunks[shown.index]}
            <div class="flex items-center gap-3 border-y bg-muted/50 px-2 py-1 text-muted-foreground">
              <code class="flex-1">{hunk.header}</code>
              {#if editable}<button type="button" data-diff-hunk-comment={hunk.index} aria-label={tr('Comment on hunk {number}', { number: hunk.index + 1 })} title={tr('Comment on hunk {number}', { number: hunk.index + 1 })}
                onclick={() => void commentHunk(hunk)} class="rounded-md p-1.5 hover:bg-muted"><MessageSquare class="size-3.5" /></button>{/if}
            </div>
            {#each shown.lines as line}
              <div class={['flex', line.kind === 'addition' ? 'bg-emerald-500/10 text-emerald-800 dark:text-emerald-200' : line.kind === 'deletion' ? 'bg-red-500/10 text-red-800 dark:text-red-200' : line.kind === 'header' ? 'text-muted-foreground' : '']}>
                {#each ['old', 'new'] as side}
                  {@const version = side as 'old' | 'new'}
                  {@const number = line[version === 'old' ? 'oldLine' : 'newLine']}
                  {#if number !== null}<button type="button" data-diff-hunk={hunk.index} data-diff-side={version} data-diff-line={number}
                    aria-label={tr(version === 'old' ? 'Old line {line}' : 'New line {line}', { line: number })}
                    aria-pressed={selection?.file_id === active.file.id && anchorIncludesLine(selection, hunk.index, version, number)}
                    onclick={(event) => selectLine(hunk.index, version, number, event.shiftKey)} onkeydown={(event) => void lineKey(event, hunk, version, number)}
                    class="w-12 shrink-0 border-r px-1 text-right text-muted-foreground hover:bg-primary/15 focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-primary aria-pressed:bg-primary/20 aria-pressed:text-foreground">{number}</button>
                  {:else}<span class="w-12 shrink-0 border-r" aria-hidden="true"></span>{/if}
                {/each}
                <span class="whitespace-pre px-2" data-diff-text>{line.text}</span>
              </div>
            {/each}
          {/each}
        </div>
      </div>
      {#if totalLines > pageSize}<div class="mt-2 flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
        <button type="button" disabled={pageStart === 0} onclick={() => pageStart = Math.max(0, pageStart - pageSize)} class="underline disabled:opacity-40">{tr('Previous lines')}</button>
        <span>{tr('Showing {first}–{last} of {total}', { first: pageStart + 1, last: Math.min(pageStart + pageSize, totalLines), total: totalLines })}</span>
        <button type="button" disabled={pageStart + pageSize >= totalLines} onclick={() => pageStart += pageSize} class="underline disabled:opacity-40">{tr('Next lines')}</button>
      </div>{/if}
    </div>
    <aside class="min-w-0" aria-label={tr('Comments')} data-tour="diff-comments">
      <h3 class="mb-3 mt-0 text-xs font-semibold">{tr('Comments')} · {current.comments.length}</h3>
      {#if !current.comments.length}<p class="m-0 text-xs leading-5 text-muted-foreground">{tr('No comments yet. You can also submit only overall feedback.')}</p>{/if}
      <div class="grid gap-2">
        {#each current.comments as comment (comment.id)}
          <button type="button" data-diff-comment={comment.id} aria-expanded={selectedId === comment.id} onclick={() => void openComment(comment)}
            class="grid min-w-0 gap-1 rounded-md border px-3 py-2 text-left text-xs hover:border-primary/50 aria-expanded:border-primary aria-expanded:bg-primary/5">
            <span class="truncate font-mono text-[10px] text-muted-foreground">{diffAnchorLabel(data.files.find((file) => file.id === comment.anchor.file_id), comment.anchor)}</span>
            <span class="truncate">{preview(comment)}</span>
          </button>
        {/each}
      </div>
      {#if selected}
        {#key selected.id}
          <div class="mt-3 grid gap-3 rounded-md border bg-background p-3" data-diff-comment-editor>
            <div class="flex min-w-0 items-center gap-2"><span class="min-w-0 flex-1 break-all font-mono text-[10px] text-muted-foreground">{diffAnchorLabel(data.files.find((file) => file.id === selected.anchor.file_id), selected.anchor)}</span>
              {#if editable}<button type="button" aria-label={tr('Delete comment')} title={tr('Delete comment')} onclick={() => void deleteComment(selected!.id)} class="rounded-md p-1.5 text-muted-foreground hover:bg-destructive/10 hover:text-destructive"><Trash2 class="size-3.5" /></button>{/if}
              <button type="button" aria-label={tr('Close comment')} title={tr('Close comment')} onclick={() => selectedId = null} class="rounded-md p-1.5 hover:bg-muted"><X class="size-3.5" /></button>
            </div>
            <WorkbenchTextField bind:this={commentField} value={selected.body} {target} maxLength={4000} disabled={!editable}
              label={tr('Comment')} voiceLabel={tr('Speak comment')} onChange={(body) => changeComment(selected!.id, body)} />
          </div>
        {/key}
      {/if}
    </aside>
  </div>
  {#if message || issue}<p class="m-0 text-xs text-amber-700 dark:text-amber-400" role="status">{message || tr(issue!)}</p>{/if}
</section>

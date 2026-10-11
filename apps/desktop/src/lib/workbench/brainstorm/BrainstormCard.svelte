<script lang="ts">
  import { tick } from 'svelte'
  import { Archive, Check, Pencil, Trash2, Undo2 } from '@lucide/svelte'
  import * as Dialog from '$lib/components/ui/dialog'
  import { Button } from '$lib/components/ui/button'
  import type { BrainstormCard, BrainstormGroup } from '../../generated/feedback'
  import type { InputTarget } from '../../domain/inputTarget'
  import { locale } from '../../preferences'
  import { isImeCompositionKey } from '../../imeComposition'
  import WorkbenchTextField from '../../input/WorkbenchTextField.svelte'
  import { useInputTools, unavailableInputToolsState } from '../../input/inputToolsContext'
  import { fieldAttachmentText } from '../../input/fieldAttachmentText'
  import { brainstormText } from './brainstormI18n'

  export let card: BrainstormCard
  export let groups: BrainstormGroup[]
  export let sourceTitles: string[] = []
  export let currentRound = false
  export let selected = false
  export let editing = false
  export let disabled = false
  export let target: InputTarget | null
  export let onUpdate: (card: BrainstormCard) => void
  export let onSelect: () => void
  export let onEdit: () => void
  export let onClose: () => void
  export let onPark: () => void
  export let removable = false
  export let onRemove: () => void = () => {}
  let confirmRemove = false
  let titleInput: HTMLInputElement
  let field: WorkbenchTextField | undefined
  const toolsState = useInputTools()?.state ?? unavailableInputToolsState
  const tr = (source: string) => brainstormText($locale, source)
  $: preview = fieldAttachmentText(card.body, $toolsState.attachments)
  $: groupTitle = groups.find((group) => group.id === card.group_id)?.title
  function change(patch: Partial<BrainstormCard>) { if (!disabled) onUpdate({ ...card, ...patch }) }
  function remove() { if (disabled || !removable) return; if (card.title.trim() || card.body.trim()) confirmRemove = true; else onRemove() }
  export async function focusBody() { await tick(); field?.focusAtEnd() }
  export async function focusTitle() { await tick(); titleInput?.focus() }
  function keydown(event: KeyboardEvent) {
    if (!editing || confirmRemove || event.defaultPrevented || isImeCompositionKey(event) || field?.isComposing()) return
    if ((event.key === 'Escape' && !event.ctrlKey && !event.metaKey && !event.altKey)
      || (event.key === 'Enter' && (event.ctrlKey || event.metaKey) && !event.altKey && !event.shiftKey)) {
      event.preventDefault(); event.stopPropagation(); onClose()
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions (Local editor commands must precede TipTap.) -->
<article class="idea-card" class:selected class:parked={card.parked} class:editing data-brainstorm-card={card.id} onkeydowncapture={keydown}>
  <div class="flex items-center gap-2 text-xs text-muted-foreground">
    <label class="inline-flex min-w-0 items-center gap-2">
      <input type="checkbox" checked={selected} disabled={disabled || card.parked} aria-label={`${tr('Select idea')}: ${card.title || tr('Untitled idea')}`} onchange={onSelect} />
      <span>{tr(card.origin === 'human' ? 'You' : 'Agent')}</span>
    </label>
    <span class="ml-auto text-[10px]">{tr(currentRound ? 'New this round' : 'Earlier round')}</span>
  </div>
  {#if editing}
    <label class="mt-3 grid gap-1.5 text-xs">{tr('Idea title')}
      <input bind:this={titleInput} value={card.title} {disabled} aria-label={tr('Idea title')} oninput={(event) => change({ title: [...event.currentTarget.value].slice(0, 200).join('') })} class="rounded-md border bg-background px-2 py-2 text-sm font-medium" />
    </label>
    <div class="my-3">
      <WorkbenchTextField bind:this={field} value={card.body} label={tr('Idea details')} voiceLabel={tr('Speak idea')} {target} maxLength={4000} {disabled}
        placeholder={tr('Add details, constraints, or examples…')} data-brainstorm-field="card_body" onChange={(body) => change({ body })} />
    </div>
    <label class="flex items-center gap-2 text-xs">{tr('Group')}
      <select value={card.group_id ?? ''} {disabled} aria-label={tr('Move to group')} onchange={(event) => change({ group_id: event.currentTarget.value || null })} class="min-w-0 flex-1 rounded-md border bg-background p-2">
        <option value="">{tr('Ungrouped')}</option>{#each groups as group}<option value={group.id}>{group.title || tr('Untitled group')}</option>{/each}
      </select>
    </label>
    <p class="mb-0 text-[11px] text-muted-foreground">{tr('Esc to return to the board. Your draft stays.')}</p>
  {:else}
    <button type="button" onclick={onEdit} class="mb-2 mt-3 block w-full text-left text-sm font-semibold leading-6" aria-label={`${tr('Edit idea')}: ${card.title || tr('Untitled idea')}`}>{card.title || tr('Untitled idea')}</button>
    {#if preview.text}<p class="m-0 line-clamp-5 whitespace-pre-wrap break-words text-xs leading-6 text-muted-foreground">{preview.text}</p>{/if}
    {#if preview.attachmentIds.length}<span class="mt-2 inline-block text-xs text-muted-foreground">{tr('Attachments')} · {preview.attachmentIds.length}</span>{/if}
    {#if groupTitle}<span class="mt-3 inline-block max-w-full truncate rounded bg-muted px-2 py-1 text-[10px]">{groupTitle}</span>{/if}
  {/if}
  {#if sourceTitles.length}<details class="mt-2 text-[10px] text-muted-foreground"><summary class="cursor-pointer">{tr('Based on')} · {sourceTitles.length}</summary><ul class="mb-0 pl-4">{#each sourceTitles as title}<li>{title}</li>{/each}</ul></details>{/if}
  <div class="mt-3 flex items-center justify-between gap-2 border-t pt-2">
    <button type="button" {disabled} onclick={onPark} class="inline-flex items-center gap-1.5 rounded p-1 text-xs text-muted-foreground hover:bg-muted disabled:opacity-40">
      {#if card.parked}<Undo2 class="size-3.5" />{:else}<Archive class="size-3.5" />{/if}{tr(card.parked ? 'Restore idea' : 'Park idea')}
    </button>
    {#if removable}<button type="button" {disabled} onclick={remove} aria-label={tr('Remove new idea')} title={tr('Remove new idea')} class="rounded p-1 text-muted-foreground hover:text-destructive"><Trash2 class="size-3.5" /></button>{/if}
    <button type="button" onclick={editing ? onClose : onEdit} class="inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs hover:bg-muted">
      {#if editing}<Check class="size-3.5" />{tr('Done')}{:else}<Pencil class="size-3.5" />{tr('Edit idea')}{/if}
    </button>
  </div>
</article>

<Dialog.Root bind:open={confirmRemove}>
  <Dialog.Content class="max-w-sm" showCloseButton={false}>
    <Dialog.Header><Dialog.Title>{tr('Remove this new idea?')}</Dialog.Title><Dialog.Description>{tr('This new card and its text will be removed. Inherited cards can only be parked.')}</Dialog.Description></Dialog.Header>
    <Dialog.Footer><Button variant="outline" onclick={() => confirmRemove = false}>{tr('Cancel')}</Button><Button variant="destructive" {disabled} onclick={() => { if (!disabled && removable) { confirmRemove = false; onRemove() } }}>{tr('Remove new idea')}</Button></Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<style>
  .idea-card { min-width: 0; align-self: start; border: 1px solid var(--border); border-radius: 10px; background: var(--background); padding: 14px; }
  .idea-card.selected { border-color: color-mix(in srgb, var(--primary) 60%, transparent); box-shadow: inset 3px 0 0 var(--primary); }
  .idea-card.parked { background: var(--muted); }
  .idea-card.editing { box-shadow: 0 4px 18px #0000000b; }
</style>

<script lang="ts">
  import { tick } from 'svelte'
  import { ArrowDown, ArrowUp, ArrowUpRight, Brain, Layers, Maximize2, Plus, Trash2 } from '@lucide/svelte'
  import type { BrainstormCard as Card, BrainstormData, BrainstormIntent, BrainstormPhase } from '../../generated/feedback'
  import type { BrainstormState } from '../definitions/brainstorm/state'
  import { locale } from '../../preferences'
  import { workbenchFieldTarget, type InputTarget } from '../../domain/inputTarget'
  import { unavailableVoiceInputState, useVoiceInput } from '../../speech/voiceInputContext'
  import WorkbenchTextField from '../../input/WorkbenchTextField.svelte'
  import BrainstormCard from './BrainstormCard.svelte'
  import { addBrainstormCard, initialBrainstormState, parkBrainstormCard, validateBrainstormState } from './brainstormModel'
  import { brainstormText } from './brainstormI18n'

  export let data: BrainstormData
  export let state: BrainstormState | null = null
  export let requestId: string
  export let disabled = false
  export let readOnly = false
  export let onChange: (state: BrainstormState) => void
  export let onOpenFeedback: (() => void) | undefined = undefined
  export let onOpenExpanded: (() => void) | undefined = undefined
  let root: HTMLElement
  let editingId: string | null = null
  const cardViews: Record<string, BrainstormCard | undefined> = {}
  let instructionField: WorkbenchTextField | undefined
  let showParked = false
  let notice = ''
  let sourceKey = ''
  let revealedSequence: number | undefined
  const voiceState = useVoiceInput()?.state ?? unavailableVoiceInputState
  const phases: BrainstormPhase[] = ['diverge', 'organize', 'focus']
  const actions: { intent: BrainstormIntent; label: string; description: string; minimum: number }[] = [
    { intent: 'explore', label: 'Keep exploring', description: 'Ask for fresh alternatives. Existing cards stay.', minimum: 0 },
    { intent: 'expand', label: 'Expand selected', description: 'Develop the selected ideas into more concrete options.', minimum: 1 },
    { intent: 'combine', label: 'Combine selected', description: 'Create new ideas that combine the selected cards.', minimum: 2 },
    { intent: 'proceed', label: 'Proceed with selected', description: 'Continue the original task using the selected direction.', minimum: 1 },
  ]
  $: current = state ?? initialBrainstormState(data)
  $: editable = !disabled && !readOnly
  $: issue = validateBrainstormState(data, current)
  $: activeCards = current.cards.filter((card) => !card.parked)
  $: parkedCards = current.cards.filter((card) => card.parked)
  $: sections = current.phase === 'organize'
    ? [...current.groups.map((group) => ({ id: group.id, title: group.title, cards: activeCards.filter((card) => card.group_id === group.id), group: true })),
      { id: '_ungrouped', title: tr('Ungrouped'), cards: activeCards.filter((card) => !card.group_id), group: false }]
    : current.phase === 'focus'
      ? [{ id: '_selected', title: tr('Selected ideas'), cards: activeCards.filter((card) => current.selected_card_ids.includes(card.id)), group: false },
        { id: '_other', title: tr('Other ideas'), cards: activeCards.filter((card) => !current.selected_card_ids.includes(card.id)), group: false }]
      : [{ id: '_ideas', title: tr('Ideas'), cards: activeCards, group: false }]
  $: if (sourceKey !== data.source_version) { sourceKey = data.source_version; editingId = null; showParked = false; notice = ''; revealedSequence = undefined }
  $: void revealField($voiceState.revealSequence, $voiceState.revealTarget)
  const tr = (source: string, values: Record<string, string | number> = {}) => brainstormText($locale, source, values)
  function update(next: BrainstormState) { if (editable) { state = next; current = next; onChange(next) } }
  function persistInitial() { if (!state && editable) update(current) }
  function target(field: string, entityId: string, label: string): InputTarget {
    return workbenchFieldTarget({ requestId, requestTitle: data.title }, { workbenchType: 'brainstorm', version: 1,
      field, entityId, sourceVersion: data.source_version, label })
  }
  async function editCard(card: Card, focus = true) {
    persistInitial(); editingId = card.id
    if (card.parked) showParked = true
    await tick()
    if (focus && editable) { if (!card.title) await cardViews[card.id]?.focusTitle(); else await cardViews[card.id]?.focusBody() }
  }
  async function closeCard() {
    const previous = editingId; editingId = null
    await tick()
    root?.querySelector<HTMLButtonElement>(`[data-brainstorm-card="${previous}"] button[aria-label]`)?.focus({ preventScroll: true })
  }
  async function addCard(groupId: string | null = null) {
    if (!editable) return
    if (current.cards.length >= 200) { notice = tr('Idea limit reached (200).'); return }
    const next = addBrainstormCard(data, current, `idea-${crypto.randomUUID()}`, groupId)
    update(next); await editCard(next.cards.at(-1)!)
  }
  function changeCard(card: Card) { update({ ...current, cards: current.cards.map((item) => item.id === card.id ? card : item) }) }
  async function selectCard(card: Card) {
    if (card.parked) return
    update({ ...current, selected_card_ids: current.selected_card_ids.includes(card.id)
      ? current.selected_card_ids.filter((id) => id !== card.id) : [...current.selected_card_ids, card.id] })
    await tick()
    root?.querySelector<HTMLInputElement>(`[data-brainstorm-card="${card.id}"] input[type="checkbox"]`)?.focus({ preventScroll: true })
  }
  async function parkCard(card: Card) {
    update(parkBrainstormCard(current, card.id, !card.parked))
    if (editingId === card.id) editingId = null
  }
  function removeNewCard(card: Card) {
    if (!editable || data.initial_board.cards.some((item) => item.id === card.id) || card.origin !== 'human' || card.origin_request_id !== data.source_version) return
    update({ ...current, cards: current.cards.filter((item) => item.id !== card.id), selected_card_ids: current.selected_card_ids.filter((id) => id !== card.id) })
    if (editingId === card.id) editingId = null
  }
  async function addGroup() {
    if (!editable) return
    if (current.groups.length >= 30) { notice = tr('Group limit reached (30).'); return }
    const id = `group-${crypto.randomUUID()}`
    update({ ...current, phase: 'organize', groups: [...current.groups, { id, title: '' }] })
    await tick(); root?.querySelector<HTMLInputElement>(`[data-brainstorm-group-title="${id}"]`)?.focus()
  }
  function removeGroup(id: string) {
    update({ ...current, groups: current.groups.filter((group) => group.id !== id), cards: current.cards.map((card) => card.group_id === id ? { ...card, group_id: null } : card) })
  }
  function moveGroup(id: string, direction: number) {
    const groups = [...current.groups], from = groups.findIndex((group) => group.id === id), to = from + direction
    if (to < 0 || to >= groups.length) return
    ;[groups[from], groups[to]] = [groups[to], groups[from]]
    update({ ...current, groups })
  }
  function moveSelected(groupId: string) {
    update({ ...current, cards: current.cards.map((card) => current.selected_card_ids.includes(card.id) ? { ...card, group_id: groupId || null } : card) })
  }
  async function revealField(sequence: number | undefined, input: InputTarget | null | undefined) {
    const destination = input?.destination
    if (sequence === undefined || sequence === revealedSequence || input?.requestId !== requestId || destination?.kind !== 'workbench_field'
      || destination.workbenchType !== 'brainstorm' || destination.version !== 1 || destination.sourceVersion !== data.source_version) return
    if (destination.field === 'card_body') {
      const card = current.cards.find((item) => item.id === destination.entityId)
      if (!card) return
      revealedSequence = sequence; await editCard(card)
    } else if (destination.field === 'instruction' && destination.entityId === 'round') {
      revealedSequence = sequence; persistInitial(); await tick(); instructionField?.focusAtEnd()
    }
  }
</script>

{#snippet cards(items: Card[])}
  <div class="idea-grid">
    {#each items as card (card.id)}
      <BrainstormCard bind:this={cardViews[card.id]} {card} groups={current.groups}
        sourceTitles={card.source_card_ids.map((id) => current.cards.find((item) => item.id === id)?.title ?? id)}
        currentRound={card.origin_request_id === data.source_version} selected={current.selected_card_ids.includes(card.id)} editing={editingId === card.id}
        disabled={!editable} target={target('card_body', card.id, card.title || tr('Untitled idea'))}
        removable={!data.initial_board.cards.some((item) => item.id === card.id) && card.origin === 'human' && card.origin_request_id === data.source_version}
        onRemove={() => removeNewCard(card)}
        onUpdate={changeCard} onSelect={() => void selectCard(card)} onEdit={() => void editCard(card)} onClose={() => void closeCard()} onPark={() => void parkCard(card)} />
    {/each}
  </div>
{/snippet}

<section bind:this={root} class="brainstorm-board p-4" data-brainstorm aria-label={tr('Brainstorm')}>
  <header class="flex flex-wrap items-center gap-3">
    <Brain class="size-5 shrink-0 text-muted-foreground" />
    <div class="min-w-0 flex-1"><h2 class="m-0 text-base font-semibold">{data.title}</h2><p class="mb-0 mt-1 text-xs text-muted-foreground">{tr('Round {round}', { round: data.round })}{#if readOnly} · {tr('Read-only board')}{/if}</p></div>
    {#if onOpenExpanded}<button type="button" onclick={onOpenExpanded} class="inline-flex items-center gap-1 rounded border px-2 py-1.5 text-xs"><Maximize2 class="size-3.5" />{tr('Full screen board')}</button>{/if}
  </header>
  <p class="my-3 text-xs leading-5 text-muted-foreground">{tr('Your ideas carry forward. New agent ideas never replace your cards.')}</p>
  <div class="flex flex-wrap items-center gap-2" data-tour="brainstorm-phase">
    <div class="inline-flex overflow-hidden rounded-lg border" role="group" aria-label={tr('Board view')}>
      {#each phases as phase}<button type="button" disabled={!editable} aria-pressed={current.phase === phase} onclick={() => update({ ...current, phase })} class="px-3 py-2 text-xs aria-pressed:bg-primary/10 aria-pressed:text-primary disabled:opacity-60">{tr(phase === 'diverge' ? 'Diverge' : phase === 'organize' ? 'Organize' : 'Focus')}</button>{/each}
    </div>
    <button type="button" disabled={!editable || current.cards.length >= 200} onclick={() => void addCard()} class="ml-auto inline-flex items-center gap-1 rounded-lg border px-3 py-2 text-xs"><Plus class="size-3.5" />{tr('Add idea')}</button>
    <button type="button" disabled={!editable || current.groups.length >= 30} onclick={() => void addGroup()} class="inline-flex items-center gap-1 rounded-lg border px-3 py-2 text-xs"><Layers class="size-3.5" />{tr('Add group')}</button>
  </div>
  <div class="my-3 flex min-h-12 flex-wrap items-center gap-2 rounded-lg bg-muted/40 p-2 text-xs" data-tour="brainstorm-selection">
    <strong>{tr('{count} selected', { count: current.selected_card_ids.length })}</strong>
    <button type="button" disabled={!editable || !current.selected_card_ids.length} onclick={() => update({ ...current, selected_card_ids: [] })} class="rounded px-2 py-1 hover:bg-muted disabled:opacity-40">{tr('Clear selection')}</button>
    <select aria-label={tr('Move selected…')} disabled={!editable || !current.selected_card_ids.length} value="_prompt" onchange={(event) => { moveSelected(event.currentTarget.value); event.currentTarget.value = '_prompt' }} class="ml-auto min-w-0 rounded border bg-background px-2 py-1.5">
      <option value="_prompt" disabled>{tr('Move selected…')}</option><option value="">{tr('Ungrouped')}</option>{#each current.groups as group}<option value={group.id}>{group.title || tr('Untitled group')}</option>{/each}
    </select>
    <button type="button" disabled={!editable || !current.selected_card_ids.length} onclick={() => update({ ...current, cards: current.cards.map((card) => current.selected_card_ids.includes(card.id) ? { ...card, parked: true } : card), selected_card_ids: [] })} class="rounded border px-2 py-1.5 disabled:opacity-40">{tr('Park selected')}</button>
  </div>
  {#if notice}<p role="status" class="text-xs text-muted-foreground">{notice}</p>{/if}
  <div class:grouped={current.phase === 'organize'} class="board-sections" data-tour="brainstorm-cards">
    {#each sections as section (section.id)}
      <section class="board-group" data-brainstorm-group={section.id}>
        <header class="mb-3 flex items-center gap-2">
          {#if section.group}
            <input aria-label={tr('Group title')} data-brainstorm-group-title={section.id} disabled={!editable} value={section.title}
              oninput={(event) => update({ ...current, groups: current.groups.map((group) => group.id === section.id ? { ...group, title: [...event.currentTarget.value].slice(0, 200).join('') } : group) })} placeholder={tr('Untitled group')} class="min-w-0 flex-1 rounded border border-transparent bg-transparent px-1 py-1 text-sm font-semibold focus:border-border" />
            <button type="button" disabled={!editable || current.groups[0]?.id === section.id} aria-label={tr('Move group earlier')} onclick={() => moveGroup(section.id, -1)} class="p-1 disabled:opacity-30"><ArrowUp class="size-3" /></button>
            <button type="button" disabled={!editable || current.groups.at(-1)?.id === section.id} aria-label={tr('Move group later')} onclick={() => moveGroup(section.id, 1)} class="p-1 disabled:opacity-30"><ArrowDown class="size-3" /></button>
            <button type="button" disabled={!editable} aria-label={tr('Remove group')} title={tr('Cards return to Ungrouped when a group is removed.')} onclick={() => removeGroup(section.id)} class="p-1 disabled:opacity-30"><Trash2 class="size-3" /></button>
          {:else}<h3 class="m-0 min-w-0 flex-1 text-sm font-semibold">{section.title}</h3>{/if}
          <span class="text-xs text-muted-foreground">{section.cards.length}</span>
        </header>
        {@render cards(section.cards)}
        {#if !section.cards.length}<p class="rounded-lg border border-dashed p-4 text-xs leading-5 text-muted-foreground">{tr(section.id === '_selected' ? 'No ideas selected. Select cards to bring them into focus.' : 'No ideas here yet. Add one or move ideas into this group.')}</p>{/if}
      </section>
    {/each}
  </div>
  <section class="my-4" data-tour="brainstorm-parked">
    <button type="button" aria-expanded={showParked} onclick={() => showParked = !showParked} class="mb-3 text-xs text-muted-foreground underline">{tr('Parked ({count})', { count: parkedCards.length })}</button>
    {#if showParked}{@render cards(parkedCards)}{#if !parkedCards.length}<p class="text-xs text-muted-foreground">{tr('No parked ideas.')}</p>{/if}{/if}
  </section>
  <section class="mt-6 rounded-xl border p-4" data-tour="brainstorm-next">
    <h3 class="m-0 text-sm font-semibold">{tr('What should the agent do next?')}</h3>
    <p class="my-2 text-xs leading-5 text-muted-foreground">{tr('This sets the next step. The agent starts after you submit the round.')}</p>
    <div class="grid gap-2 sm:grid-cols-2">
      {#each actions as action}<button type="button" data-brainstorm-intent={action.intent} disabled={!editable || current.selected_card_ids.length < action.minimum} aria-pressed={current.intent === action.intent}
        onclick={() => update({ ...current, intent: action.intent })} class="rounded-lg border p-3 text-left aria-pressed:border-primary aria-pressed:bg-primary/5 disabled:opacity-40"><strong class="block text-xs">{tr(action.label)}</strong><span class="mt-1 block text-[11px] leading-5 text-muted-foreground">{tr(action.description)}</span></button>{/each}
    </div>
    <div class="my-3" onfocusin={persistInitial}>
      <WorkbenchTextField bind:this={instructionField} value={current.instruction} label={tr('Instructions for the next step (optional)')} voiceLabel={tr('Speak next-step instructions')}
        target={target('instruction', 'round', tr('Instructions for the next step (optional)'))} maxLength={4000} disabled={!editable} data-brainstorm-field="instruction" onChange={(instruction) => update({ ...current, instruction })} />
    </div>
    {#if issue && current.intent}<p role="status" class="text-xs text-amber-700 dark:text-amber-400">{tr(issue)}</p>{/if}
    {#if onOpenFeedback}<button type="button" onclick={onOpenFeedback} class="inline-flex items-center gap-2 rounded-lg bg-primary px-3 py-2 text-xs text-primary-foreground"><ArrowUpRight class="size-3.5" />{tr('Review and send this round')}</button>{/if}
  </section>
</section>

<style>
  .brainstorm-board { min-width: 0; container-type: inline-size; }
  .idea-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 260px), 1fr)); gap: 12px; }
  .board-sections { display: grid; gap: 20px; }
  .board-group { min-width: 0; }
  .grouped { grid-template-columns: repeat(auto-fit, minmax(min(100%, 280px), 1fr)); align-items: start; }
  .grouped .board-group { border-radius: 10px; padding: 12px; background: color-mix(in srgb, var(--muted) 40%, transparent); }
  .grouped .idea-grid { grid-template-columns: minmax(0, 1fr); }
</style>

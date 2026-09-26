<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { readable } from 'svelte/store'
  import { CircleHelp } from '@lucide/svelte'
  import portrait from '../../../assets/rambelle-states/toast-info.webp'
  import { locale } from '$lib/preferences'
  import { unavailableVoiceInputState, useVoiceInput } from '../../speech/voiceInputContext'
  import { emptyRequestSpeechTidy } from '../../speech/requestSpeechTidy'
  import { useRequestSpeechTools } from '../../speech/requestSpeechToolsContext'
  import SpotlightTour from './SpotlightTour.svelte'
  import { getWorkbenchTour, type WorkbenchTour } from './workbenchTours'
  import { hasSeenWorkbenchTour, markWorkbenchTourSeen } from './workbenchTourSeen'

  export let kind: string
  export let requestId: string
  export let disabled = false
  export let scope: HTMLElement | undefined = undefined

  const voiceState = useVoiceInput()?.state ?? unavailableVoiceInputState
  const tidyState = useRequestSpeechTools() ?? readable(emptyRequestSpeechTidy)

  let mounted = false
  let open = false
  let visitKey = ''
  let considered = false
  let generation = 0
  let pendingModal: MutationObserver | undefined

  $: tour = getWorkbenchTour(kind, $locale)
  $: locked = disabled || $voiceState.recording || $tidyState.busy
  $: considerVisit(tour, requestId, locked, scope, mounted)

  function cancelPending() {
    generation += 1
    pendingModal?.disconnect()
    pendingModal = undefined
  }

  function openWhenAvailable(pending: number) {
    if (pending !== generation || !mounted || locked) return
    const otherModal = Array.from(document.querySelectorAll('dialog[open], [role="dialog"][aria-modal="true"]'))
      .some((element) => !element.closest('[hidden], [aria-hidden="true"]'))
    if (otherModal) {
      pendingModal ??= new MutationObserver(() => openWhenAvailable(pending))
      pendingModal.observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ['open', 'aria-modal', 'aria-hidden', 'hidden'] })
      return
    }
    pendingModal?.disconnect()
    pendingModal = undefined
    void tick().then(() => {
      if (pending !== generation || !mounted || locked) return
      considered = true
      open = true
    })
  }

  function considerVisit(guide: WorkbenchTour | null, request: string, locked: boolean, root: HTMLElement | undefined, ready: boolean) {
    const key = `${request}:${guide?.id ?? ''}:${guide?.version ?? ''}`
    if (key !== visitKey) {
      visitKey = key
      considered = false
      open = false
      cancelPending()
    }
    if (locked || !guide || !root || !ready) {
      open = false
      cancelPending()
      return
    }
    if (considered || !request) return
    if (hasSeenWorkbenchTour(guide.id, guide.version)) return
    cancelPending()
    const pending = ++generation
    // Sibling landmarks must be mounted before the first spotlight is measured.
    void tick().then(() => openWhenAvailable(pending))
  }

  function dismiss() {
    if (tour) markWorkbenchTourSeen(tour.id, tour.version)
    open = false
    cancelPending()
  }

  function replay() {
    if (!tour || locked || !scope) return
    cancelPending()
    openWhenAvailable(generation)
  }

  onMount(() => {
    mounted = true
    return () => { mounted = false; cancelPending() }
  })
</script>

{#if tour}
  <button type="button" data-workbench-guide-trigger disabled={locked || !scope}
    class="inline-flex h-7 shrink-0 items-center gap-1.5 rounded-md px-2 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-40"
    aria-label={tour.replayLabel} title={tour.replayLabel} onclick={replay}>
    <CircleHelp size={14} />
    <span>{tour.replayLabel}</span>
  </button>
  {#if open && scope && !locked}
    {#key visitKey}
      <SpotlightTour steps={tour.steps} labels={tour.labels} {scope} {portrait} onDismiss={dismiss} />
    {/key}
  {/if}
{/if}

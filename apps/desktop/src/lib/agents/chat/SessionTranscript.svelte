<!-- Reverse-scroll trigger adapted from Codeg virtualized-message-thread at 3ebdfed. -->
<!-- SPDX-License-Identifier: Apache-2.0; Svelte windows and durable activity anchors. -->
<script lang="ts">
  import { onDestroy, tick } from 'svelte'
  import { LoaderCircle, MessageSquare } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import { locale } from '$lib/preferences'
  import { agentText } from '../agentI18n'
  import { redactAgentMessage } from '../agentConfigForm'
  import { activitiesForSession, type SessionActivity } from '../managedSessionUi'
  import { TimelineWindow, crossedHistoryThreshold } from './timeline-window'
  import { captureActivityAnchor, restoreActivityAnchor } from './scroll-anchor'
  import { chatText } from './chat-text'
  import SessionTimeline from './SessionTimeline.svelte'

  export let sessionId: string
  export let activities: readonly SessionActivity[]
  export let runActive = false
  export let historyLoading = false
  export let historyHasMore = false
  export let historyError = ''
  export let onLoadOlder: (() => Promise<void> | void) | undefined = undefined
  export let envText = ''

  const timelineWindow = new TimelineWindow()
  let viewport: HTMLDivElement | undefined
  let activeSessionId = ''
  let stickToBottom = true
  let previousScrollTop: number | null = null
  let destroyed = false
  let windowRevision = 0
  let historyRequestId = ''
  let localError = ''
  let renderedActivities: readonly SessionActivity[] = []
  $: if (activeSessionId !== sessionId) {
    activeSessionId = sessionId
    stickToBottom = true
    previousScrollTop = null
    localError = ''
  }
  $: visibleActivities = activitiesForSession(sessionId, activities)
  $: {
    windowRevision
    renderedActivities = timelineWindow.read(sessionId, visibleActivities, stickToBottom)
  }
  $: localHistory = renderedActivities.length < visibleActivities.length
  $: loadingHistory = historyLoading || historyRequestId === sessionId
  $: visibleError = redactAgentMessage(localError || historyError, envText)
  $: if (visibleActivities.length) void followActivity(visibleActivities)

  function tr(source: string) {
    if ($locale === 'zh-CN' && source === 'Describe what you want to work on.') return '描述你想完成的任务。'
    return chatText($locale, agentText($locale, source))
  }
  async function followActivity(_activities: readonly SessionActivity[]) {
    const id = sessionId
    await tick()
    if (!destroyed && id === sessionId && stickToBottom && viewport) {
      viewport.scrollTop = viewport.scrollHeight
      previousScrollTop = viewport.scrollTop
    }
  }
  function rememberScroll() {
    if (!viewport) return
    const top = viewport.scrollTop
    const load = crossedHistoryThreshold(previousScrollTop, top)
    previousScrollTop = top
    stickToBottom = viewport.scrollHeight - top - viewport.clientHeight < 80
    if (load && !visibleError) void loadEarlier()
  }
  async function loadEarlier() {
    if (loadingHistory || (!localHistory && ((!historyHasMore && !visibleError) || !onLoadOlder)) || !viewport) return
    const id = sessionId
    const element = viewport
    // Capture before the fetch: a partial first turn can expand immediately
    // when its earlier rows arrive, before the local window is revealed.
    const anchor = captureActivityAnchor(element)
    historyRequestId = id
    localError = ''
    stickToBottom = false
    try {
      if ((!localHistory || visibleError) && onLoadOlder) { await onLoadOlder(); await tick() }
      if (destroyed || sessionId !== id) return
      timelineWindow.revealOlder(visibleActivities)
      windowRevision += 1
      await tick()
      if (destroyed || sessionId !== id) return
      if (anchor) {
        restoreActivityAnchor(element, anchor)
        const restoredTop = element.scrollTop
        requestAnimationFrame(() => {
          if (!destroyed && sessionId === id && Math.abs(element.scrollTop - restoredTop) < 1) {
            restoreActivityAnchor(element, anchor)
            previousScrollTop = element.scrollTop
          }
        })
      }
      previousScrollTop = element.scrollTop
    } catch { localError = tr('Could not load earlier messages.') }
    finally { if (historyRequestId === id) historyRequestId = '' }
  }
  onDestroy(() => { destroyed = true })
</script>

<div bind:this={viewport} onscroll={rememberScroll} class="min-h-0 flex-1 space-y-5 overflow-y-auto overscroll-contain px-5 py-5" aria-label={tr('Session activity')} data-session-transcript>
  {#if localHistory || historyHasMore || visibleError}
    <div class="mx-auto flex max-w-4xl flex-col items-center gap-2 pb-1">
      <Button variant="ghost" size="sm" disabled={loadingHistory || (!localHistory && !onLoadOlder)} onclick={() => void loadEarlier()}>{#if loadingHistory}<LoaderCircle class="size-3.5 animate-spin" />{/if}{tr(loadingHistory ? 'Loading earlier messages…' : 'Load earlier messages')}</Button>
      {#if visibleError}<p class="m-0 text-xs text-destructive" role="alert">{visibleError}</p>{/if}
    </div>
  {/if}
  <SessionTimeline {sessionId} activities={renderedActivities} {runActive} onResize={() => void followActivity(visibleActivities)} />
  {#if visibleActivities.length === 0}
    <div class="mx-auto flex min-h-48 max-w-md flex-col items-center justify-center text-center"><MessageSquare class="mb-3 size-6 text-muted-foreground/50" /><strong class="text-sm font-medium">{tr('No messages yet')}</strong><p class="mb-0 mt-2 text-xs leading-5 text-muted-foreground">{tr('Describe what you want to work on.')}</p></div>
  {/if}
</div>

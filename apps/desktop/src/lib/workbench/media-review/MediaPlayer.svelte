<script lang="ts">
  import { onMount } from 'svelte'
  import type { MediaReviewData } from '../../generated/feedback'
  import type { MediaReviewController } from './mediaController'
  import { mediaDurationMismatch } from './mediaModel'
  import { locale } from '../../preferences'
  import { mediaReviewText } from './mediaI18n'
  export let data: MediaReviewData
  export let runtime: MediaReviewController | undefined
  export let onClock: (ms: number) => void = () => {}
  export let onReady: (ready: boolean) => void = () => {}
  let element: HTMLMediaElement, loading = true, message = '', loaded = false, pendingSeek: number | null = null
  let active = true, sequence = 0
  let binding: ReturnType<MediaReviewController['bindPlayer']> | undefined
  const tr = (source: string) => mediaReviewText($locale, source)
  export function pause() { element?.pause() }
  export function seek(ms: number) {
    if (!loaded) { pendingSeek = ms; return }
    if (ms > element.duration * 1000 + 1) { message = 'The selected time is outside the playable media. The comment anchor is unchanged.'; return }
    element.currentTime = Math.max(0, ms) / 1000
    onClock(Math.max(0, Math.round(element.currentTime * 1000)))
  }
  async function load() {
    const attempt = ++sequence
    loading = true; message = ''; loaded = false; onReady(false)
    try {
      if (!runtime) throw new Error('The original media attachment is unavailable.')
      const source = await runtime.loadSource(data)
      if (!active || attempt !== sequence || !binding?.active()) return
      element.src = source.url; element.load(); loading = false
    } catch (error) {
      if (active && attempt === sequence) { loading = false; message = error instanceof Error ? error.message : 'The original media attachment is unavailable.' }
    }
  }
  function metadata() {
    if (!active || !binding?.active()) return
    loaded = true
    const mismatch = mediaDurationMismatch(element.duration, data.duration_ms)
    message = mismatch ? 'The media duration differs from the request. Existing timestamps are preserved. Ask the Agent to check the material and create a corrected request.' : ''
    onReady(!mismatch)
    seek(pendingSeek ?? runtime?.position() ?? 0); pendingSeek = null
  }
  function failed() {
    if (!active) return
    loading = false; loaded = false; onReady(false)
    message = 'This browser cannot play the media. Your draft is preserved. Ask for a supported media file in a new request.'
  }
  onMount(() => {
    binding = runtime?.bindPlayer(element)
    void load()
    return () => { active = false; sequence += 1; element.pause(); binding?.release() }
  })
</script>

<div class="overflow-hidden rounded-lg border bg-muted/30" data-media-player>
  <div class={data.media_kind === 'video' ? 'flex min-h-40 items-center justify-center bg-black' : 'flex min-h-28 items-center justify-center p-5'}>
    <!-- Source captions are not supplied by this immutable media contract. -->
    <svelte:element this={data.media_kind === 'video' ? 'video' : 'audio'} bind:this={element} controls playsinline preload="metadata"
      class={data.media_kind === 'video' ? 'max-h-[55vh] w-full object-contain' : 'w-full'}
      aria-label={data.title} onloadedmetadata={metadata} onerror={failed}
      ontimeupdate={() => { if (active && binding?.active()) onClock(Math.max(0, Math.round(element.currentTime * 1000))) }} />
  </div>
  {#if loading || message}<div class="flex flex-wrap items-center gap-3 border-t px-4 py-3 text-xs leading-5" role="status">
    <span class="min-w-0 flex-1">{loading ? tr('Loading media…') : tr(message)}</span>
    {#if !loading && !loaded}<button type="button" class="shrink-0 underline" onclick={() => void load()}>{tr('Retry loading')}</button>{/if}
  </div>{/if}
</div>

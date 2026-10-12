<script lang="ts">
  import { onDestroy, tick } from 'svelte'
  import type { SiteContent } from '../content/site'

  let { content: c, lang }: { content: SiteContent['workbenches']; lang: string } = $props()
  const instance = $props.id()
  let selected = $state(0)
  let requested = $state(false)
  let playing = $state(false)
  let wantsPlayback = $state(false)
  let failed = $state(false)
  let video: HTMLVideoElement | undefined = $state()
  const tabs: HTMLButtonElement[] = $state([])
  const item = $derived(c.items[selected] ?? c.items[0])
  const locale = $derived(lang === 'zh-CN' ? 'zh-CN' : 'en')
  const mediaBase = $derived(`/assets/workbenches/${locale}/${item.id}`)
  let epoch = 0
  let mediaKey = ''

  function pause() {
    epoch++
    wantsPlayback = false
    playing = false
    video?.pause()
  }

  // A new recording or language starts from its poster, even if play() is still pending.
  $effect.pre(() => {
    const key = mediaBase
    if (key === mediaKey) return
    mediaKey = key
    pause()
    requested = false
    failed = false
  })
  onDestroy(pause)

  function select(index: number) {
    if (index === selected) return
    pause()
    requested = false
    failed = false
    selected = index
  }

  function onTabKey(event: KeyboardEvent, index: number) {
    let next: number
    if (event.key === 'ArrowRight') next = (index + 1) % c.items.length
    else if (event.key === 'ArrowLeft') next = (index + c.items.length - 1) % c.items.length
    else if (event.key === 'Home') next = 0
    else if (event.key === 'End') next = c.items.length - 1
    else return
    event.preventDefault()
    select(next)
    tabs[next]?.focus()
  }

  function showFailure(target: HTMLVideoElement) {
    if (target !== video) return
    pause()
    failed = true
    requested = false
  }

  async function togglePlayback() {
    if (playing || wantsPlayback) { pause(); return }
    const attempt = ++epoch
    const key = mediaBase
    failed = false
    requested = true
    wantsPlayback = true
    await tick()
    const target = video
    if (!target || attempt !== epoch || key !== mediaBase) return
    try {
      await target.play()
      if (attempt !== epoch || key !== mediaBase) target.pause()
    } catch {
      // Navigation, pause and teardown invalidate their own pending play promises.
      if (attempt === epoch && key === mediaBase) showFailure(target)
    }
  }

  function playbackChanged(event: Event, active: boolean) {
    if (event.currentTarget !== video) return
    playing = active
    wantsPlayback = false
  }
</script>

<div class="workbench-demos">
  <div class="demo-tabs" role="tablist" aria-label={c.title}>
    {#each c.items as entry, index (entry.id)}
      <button
        bind:this={tabs[index]}
        type="button"
        role="tab"
        id={`${instance}-tab-${entry.id}`}
        aria-controls={`${instance}-panel-${entry.id}`}
        aria-selected={selected === index}
        tabindex={selected === index ? 0 : -1}
        onclick={() => select(index)}
        onkeydown={(event) => onTabKey(event, index)}
      >{entry.tab}</button>
    {/each}
  </div>
  <div
    class="demo-panel"
    role="tabpanel"
    id={`${instance}-panel-${item.id}`}
    aria-labelledby={`${instance}-tab-${item.id}`}
    tabindex="0"
  >
    <div class="demo-caption">
      {#if item.development}<span class="development-badge">{c.developmentLabel}</span>{/if}
      <h3>{item.title}</h3>
      <p id={`${instance}-description`}>{item.body}</p>
      <button class="play-control" type="button" onclick={togglePlayback} aria-controls={`${instance}-media`}>
        <span aria-hidden="true">{playing || wantsPlayback ? 'Ⅱ' : '▷'}</span>
        {playing || wantsPlayback ? c.pause : c.play}
      </button>
    </div>
    <div class="media-frame" id={`${instance}-media`} style:aspect-ratio={`${item.width} / ${item.height}`}>
      {#if requested && !failed}
        <video
          bind:this={video}
          src={`${mediaBase}.mp4`}
          poster={`${mediaBase}.webp`}
          width={item.width}
          height={item.height}
          controls
          muted
          playsinline
          preload="none"
          aria-label={item.alt}
          aria-describedby={`${instance}-description`}
          onplay={(event) => playbackChanged(event, true)}
          onpause={(event) => playbackChanged(event, false)}
          onended={(event) => playbackChanged(event, false)}
          onerror={(event) => showFailure(event.currentTarget)}
        ></video>
      {:else}
        <img src={`${mediaBase}.webp`} alt={item.alt} width={item.width} height={item.height} loading="lazy" decoding="async" />
      {/if}
    </div>
    {#if failed}
      <div class="media-error" role="alert">
        <p>{c.errorLabel}</p>
        <a href={`${mediaBase}.mp4`}>{c.openVideo} <span aria-hidden="true">↗</span></a>
      </div>
    {/if}
    <noscript>
      <div class="media-error">
        {#each c.items as entry (entry.id)}
          <p><a href={`/assets/workbenches/${locale}/${entry.id}.mp4`}>{c.openVideo}: {entry.tab}</a></p>
        {/each}
      </div>
    </noscript>
  </div>
  {#each c.items as entry (entry.id)}
    {#if entry.id !== item.id}
      <div role="tabpanel" id={`${instance}-panel-${entry.id}`} aria-labelledby={`${instance}-tab-${entry.id}`} hidden></div>
    {/if}
  {/each}
</div>

<style>
  .workbench-demos{width:100%;color:#182b49;background:#f8fbff;border:1px solid #d0ddec;border-radius:20px;overflow:hidden;box-shadow:0 24px 70px #061b3333}
  .demo-tabs{display:flex;gap:4px;padding:10px;background:#eaf1f9;border-bottom:1px solid #cbd9e9;overflow-x:auto}
  .demo-tabs button{flex:1 0 auto;padding:12px 20px;border:1px solid transparent;border-radius:10px;background:transparent;color:#405777;font:inherit;font-size:.9rem;font-weight:650;white-space:nowrap;cursor:pointer}
  .demo-tabs button[aria-selected="true"]{color:#164e9f;background:#fff;border-color:#c6d9ef;box-shadow:0 2px 6px #29496e0a}
  .demo-tabs button:hover{background:#fff9}
  .demo-caption{padding:28px 32px 24px;background:#f8fbff}
  .demo-caption h3{margin:0 0 10px;font-size:clamp(1.15rem,2vw,1.5rem);line-height:1.35;letter-spacing:-.015em}
  .demo-caption p{max-width:75ch;margin:0 0 18px;color:#4c607d;font-size:.97rem;line-height:1.65}
  .development-badge{display:inline-block;margin:0 0 12px;padding:4px 9px;border-radius:5px;background:#e4eefb;color:#25518b;font-size:.72rem;font-weight:650;line-height:1.4}
  .play-control{display:inline-flex;align-items:center;gap:9px;min-height:40px;padding:9px 16px;border:1px solid #b6cdeb;border-radius:8px;background:#fff;color:#164e9f;font:inherit;font-size:.88rem;font-weight:650;cursor:pointer}
  .play-control:hover{background:#eaf2ff}
  .play-control span{font-size:1.05rem;line-height:1}
  .media-frame{width:100%;background:#e7edf5;border-top:1px solid #d0ddec}
  .media-frame img,.media-frame video{display:block;width:100%;height:100%;object-fit:contain}
  .media-error{padding:16px 32px;margin:0;background:#fff8ed;color:#674e29;font-size:.9rem;line-height:1.6}
  .media-error p{margin:0 0 6px}
  .media-error a{color:#164e9f;text-decoration:underline;text-underline-offset:3px}
  button:focus-visible,a:focus-visible,.demo-panel:focus-visible{outline:3px solid #4286db;outline-offset:-3px}
  @media(max-width:600px){.workbench-demos{border-radius:12px}.demo-tabs{padding:7px}.demo-tabs button{padding:10px 13px;font-size:.82rem}.demo-caption{padding:20px}.media-error{padding:16px 20px}}
</style>

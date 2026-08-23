<script lang="ts">
  import { onDestroy, onMount } from 'svelte'
  import { gsap } from 'gsap'
  import AskWorkShot from './AskWorkShot.svelte'
  import RambleShot from './RambleShot.svelte'
  import ContinueShot from './ContinueShot.svelte'
  import LensNav from './LensNav.svelte'
  import type { SiteContent } from '../../content/site'

  export let content: SiteContent['lens2']

  let root: HTMLDivElement
  let askRef: AskWorkShot
  let rambleRef: RambleShot
  let contRef: ContinueShot

  const byId = (id: string) => content.shots.find((s) => s.id === id)!
  const plays: Array<() => void> = [
    () => askRef?.play(),
    () => rambleRef?.play(),
    () => contRef?.play(),
  ]
  const LAYERS = ['.layer-ask', '.layer-ramble', '.layer-cont']
  // per-lens dwell: ask play, ramble play + send, continue play
  const DWELL = [4200, 3400, 3600]

  let cur = 0
  let auto: number | null = null
  let ctx: gsap.Context | null = null

  // wipe every lens, show target, fire its play — one render, any source
  function render(i: number, immediate = false) {
    if (!root) return
    const idx = ((i % LAYERS.length) + LAYERS.length) % LAYERS.length
    const show = LAYERS[idx]
    LAYERS.forEach((sel) => {
      gsap.killTweensOf(sel)
      if (sel === show) {
        gsap.fromTo(
          sel,
          { opacity: 0, y: 36, scale: 0.975 },
          { opacity: 1, y: 0, scale: 1, duration: immediate ? 0.01 : 0.35, ease: 'power2.out' },
        )
      } else {
        gsap.to(sel, { opacity: 0, y: -30, scale: 0.975, duration: 0.25, ease: 'power2.in' })
      }
    })
    cur = idx
    plays[idx]()
  }

  function clearAuto() {
    if (auto !== null) {
      window.clearTimeout(auto)
      auto = null
    }
  }

  function scheduleNext() {
    clearAuto()
    // ramble ends with an automatic send (seal animation), then continues
    auto = window.setTimeout(() => {
      if (cur === 1) {
        rambleRef?.send()
      } else {
        render(cur + 1)
        scheduleNext()
      }
    }, DWELL[cur])
  }

  function onSent() {
    clearAuto()
    render(2)
    scheduleNext()
  }

  function jumpTo(i: number) {
    clearAuto()
    render(i, true)
    scheduleNext()
  }

  onMount(() => {
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    const narrow = window.matchMedia('(max-width: 860px)').matches
    if (reduced || narrow) return

    ctx = gsap.context(() => {
      gsap.set('.shot-layer', { opacity: 0, y: 30, scale: 0.985 })
      render(0, true)
    }, root)
    scheduleNext()
  })

  onDestroy(() => {
    clearAuto()
    ctx?.revert()
  })
</script>

<div class="lens2" bind:this={root} id="loop" aria-label={content.tagline}>
  <div class="lens2-sticky">
    <div class="stage-bg" aria-hidden="true"></div>
    <div class="stage-desk" aria-hidden="true"></div>

    <div class="shot-layer layer-ask">
      <div class="lens2-cap">
        <span class="cap-tag">{byId('ask').tag}</span>
        <h3>{byId('ask').title}</h3>
        <p>{byId('ask').sub}</p>
      </div>
      <AskWorkShot {content} bind:this={askRef} />
    </div>

    <div class="shot-layer layer-ramble">
      <div class="lens2-cap">
        <span class="cap-tag">{byId('ramble').tag}</span>
        <h3>{byId('ramble').title}</h3>
        <p>{byId('ramble').sub}</p>
      </div>
      <RambleShot {content} onsent={onSent} bind:this={rambleRef} />
    </div>

    <div class="shot-layer layer-cont">
      <div class="lens2-cap">
        <span class="cap-tag">{byId('continue').tag}</span>
        <h3>{byId('continue').title}</h3>
        <p>{byId('continue').sub}</p>
      </div>
      <ContinueShot {content} bind:this={contRef} />
    </div>

    <LensNav tags={content.shots.map((s) => s.tag)} index={cur} onjump={jumpTo} />
  </div>
</div>

<style>
  .lens2 {
    position: relative;
    height: 100svh;
    background: #050d18;
    color: #dcebfa;
  }

  .lens2-sticky {
    position: relative;
    height: 100svh;
    overflow: hidden;
    isolation: isolate;
  }

  .stage-bg {
    position: absolute;
    inset: 0;
    z-index: -3;
    background:
      radial-gradient(120% 90% at 50% 118%, rgb(39 117 202 / 15%), transparent 56%),
      radial-gradient(80% 60% at 18% 8%, rgb(87 198 192 / 7%), transparent 60%),
      linear-gradient(180deg, #03080f 0%, #071425 46%, #0a1a2e 72%, #050d18 100%);
  }

  .stage-bg::before {
    position: absolute;
    inset: 0;
    background-image: url('/assets/rambelle-vault-pattern.webp');
    background-size: 380px 380px;
    opacity: 0.5;
    filter: invert(1) hue-rotate(180deg) brightness(0.62) contrast(1.05);
    mask-image: radial-gradient(95% 85% at 50% 45%, #000 28%, transparent 80%);
    content: '';
  }

  .stage-desk {
    position: absolute;
    inset: 0;
    z-index: -2;
  }

  .stage-desk::before {
    position: absolute;
    right: 4%;
    bottom: 0;
    left: 4%;
    height: 1px;
    background: linear-gradient(90deg, transparent, rgb(87 198 192 / 36%), rgb(242 160 61 / 26%), transparent);
    box-shadow: 0 0 24px rgb(87 198 192 / 20%);
    content: '';
  }

  .stage-desk::after {
    position: absolute;
    right: 0;
    bottom: 0;
    left: 0;
    height: 34%;
    background: linear-gradient(180deg, transparent, rgb(39 117 202 / 9%) 40%, rgb(3 8 15 / 78%));
    content: '';
  }

  .shot-layer {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    will-change: transform, opacity;
  }

  .lens2-cap {
    position: absolute;
    bottom: clamp(26px, 8vh, 84px);
    left: clamp(22px, 6vw, 88px);
    z-index: 3;
    max-width: min(340px, 78vw);
    pointer-events: none;
  }

  .cap-tag {
    display: block;
    margin-bottom: 10px;
    color: #f2c77e;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.78rem;
    font-weight: 800;
    letter-spacing: 0.22em;
  }

  .lens2-cap h3 {
    margin: 0;
    color: #f2f8ff;
    font-size: clamp(1.5rem, 2.6vw, 2.2rem);
    line-height: 1.08;
    text-wrap: balance;
  }

  .lens2-cap p {
    margin: 12px 0 0;
    color: rgb(214 232 249 / 66%);
    font-size: 0.98rem;
    line-height: 1.6;
  }

  @media (max-width: 860px) {
    .lens2 {
      height: auto;
    }

    .lens2-sticky {
      position: static;
      height: auto;
      overflow: visible;
      display: grid;
      gap: 54px;
      padding: 110px 18px 60px;
    }

    .shot-layer {
      position: relative;
      inset: auto;
      display: grid;
      gap: 30px;
      min-height: 92svh;
    }

    .lens2-cap {
      bottom: auto;
      top: 0;
      left: 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .lens2 {
      height: auto;
    }

    .lens2-sticky {
      position: static;
      height: auto;
      overflow: visible;
      display: grid;
      gap: 54px;
      padding: 110px 18px 60px;
    }

    .shot-layer {
      position: relative;
      inset: auto;
      display: grid;
      gap: 30px;
      min-height: 92svh;
    }

    .lens2-cap {
      bottom: auto;
      top: 0;
      left: 0;
    }
  }
</style>

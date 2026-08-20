<script lang="ts">
  import { onMount } from 'svelte'

  type VariantKey = 'A' | 'B' | 'C' | 'D'

  type Variant = {
    name: string
    eyebrow: string
    headline: string[]
    body: string
    primary: string
    secondary: string
    note: string
  }

  const variants: Record<VariantKey, Variant> = {
    A: {
      name: 'Quiet command',
      eyebrow: 'Local-first human feedback for coding agents',
      headline: ['Stop vibe coding.', 'Rambling is all you need.'],
      body: 'Turn messy human feedback into something your coding agent can continue from.',
      primary: 'Download',
      secondary: 'Watch the happy path',
      note: 'Classic left-copy hero. Rambelle waits; the user initiates.',
    },
    B: {
      name: 'Instruction desk',
      eyebrow: 'RambleDesk is waiting for your judgment',
      headline: ['Tell the agent', 'what you actually saw.'],
      body: 'Speak, screenshot, annotate, and hand back a feedback package instead of a polished prompt.',
      primary: 'Start with a ramble',
      secondary: 'See the loop',
      note: 'Command-forward copy. The CTA behaves like giving an instruction.',
    },
    C: {
      name: 'Secretary crop',
      eyebrow: 'When the agent knocks',
      headline: ['Your secretary', 'for human feedback.'],
      body: 'RambleDesk receives your ramble, seals the evidence, and lets the original host continue.',
      primary: 'Get RambleDesk',
      secondary: 'View recording slot',
      note: 'Most character-led. Less explanatory, more immediate brand memory.',
    },
    D: {
      name: 'Static C plate',
      eyebrow: 'When the agent knocks',
      headline: ['Rambling is all you need.'],
      body: 'RambleDesk receives your ramble, seals the evidence, and lets the original host continue.',
      primary: 'Get RambleDesk',
      secondary: 'View recording slot',
      note: 'Original C hero plate as one static background. No layer split, no baking, no motion.',
    },
  }

  const orderedVariants: VariantKey[] = ['A', 'B', 'C', 'D']
  const sceneSrc = '/assets/prototypes/hero-c/scene-plate.png'
  const rambelleSrc = '/assets/prototypes/hero-c/rambelle-secretary-cutout.png'
  const originalHeroSrc = '/assets/prototypes/hero-c/original-workbench-cinema.png'
  const releaseUrl = 'https://github.com/l1veIn/rambledesk/releases'

  let variant: VariantKey = 'A'
  let pointerX = 0.58
  let pointerY = 0.42
  let awake = false
  let intent = 'waiting'
  let showControls = true
  let wakeTimer: ReturnType<typeof setTimeout> | undefined

  $: current = variants[variant]
  $: styleVars = `--px: ${pointerX}; --py: ${pointerY}; --awake: ${awake ? 1 : 0};`
  $: stateLabel = `${variant} ${current.name} / ${intent} / pointer ${Math.round(pointerX * 100)},${Math.round(pointerY * 100)}`
  $: isStaticPlate = variant === 'D'

  function parseVariant(value: string | null): VariantKey {
    if (value === 'B' || value === 'C' || value === 'D') return value
    return 'A'
  }

  function setVariant(next: VariantKey) {
    variant = next
    intent = `variant:${next}`
    if (typeof window === 'undefined') return
    const url = new URL(window.location.href)
    url.searchParams.set('variant', next)
    window.history.replaceState({}, '', url)
  }

  function cycleVariant(delta: number) {
    const index = orderedVariants.indexOf(variant)
    const nextIndex = (index + delta + orderedVariants.length) % orderedVariants.length
    setVariant(orderedVariants[nextIndex])
  }

  function wake(nextIntent: string) {
    intent = nextIntent
    awake = true
    if (wakeTimer) clearTimeout(wakeTimer)
    wakeTimer = setTimeout(() => {
      awake = false
      intent = 'waiting'
    }, 2400)
  }

  function handlePointer(event: PointerEvent) {
    const target = event.currentTarget as HTMLElement
    const rect = target.getBoundingClientRect()
    pointerX = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width))
    pointerY = Math.min(1, Math.max(0, (event.clientY - rect.top) / rect.height))
  }

  function handleKeydown(event: KeyboardEvent) {
    const target = event.target
    if (
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement ||
      (target instanceof HTMLElement && target.isContentEditable)
    ) {
      return
    }
    if (event.key === 'ArrowLeft') cycleVariant(-1)
    if (event.key === 'ArrowRight') cycleVariant(1)
  }

  onMount(() => {
    const url = new URL(window.location.href)
    variant = parseVariant(url.searchParams.get('variant'))
    showControls = url.searchParams.get('controls') !== '0'
    return () => {
      if (wakeTimer) clearTimeout(wakeTimer)
    }
  })
</script>

<svelte:window on:keydown={handleKeydown} />

<main class={`hero-c-prototype variant-${variant.toLowerCase()}`} style={styleVars}>
  <section class="hero-screen" aria-label="RambleDesk hero prototype" on:pointermove={handlePointer}>
    {#if isStaticPlate}
      <img class="static-hero-plate" src={originalHeroSrc} alt="" aria-hidden="true" />
      <div class="static-copy-wash" aria-hidden="true"></div>
    {:else}
      <img class="scene-plate" src={sceneSrc} alt="" aria-hidden="true" />
      <div class="scene-haze" aria-hidden="true"></div>
      <div class="ambient-grid" aria-hidden="true"></div>

      <svg class="command-beam" viewBox="0 0 1000 620" aria-hidden="true" preserveAspectRatio="none">
        <path class="beam-soft" d="M120 395 C310 380 432 440 548 416 C664 392 706 300 820 286" />
        <path class="beam-hot" d="M120 395 C310 380 432 440 548 416 C664 392 706 300 820 286" />
      </svg>

      <div class="rambelle-rig">
        <img class="rambelle-reflection" src={rambelleSrc} alt="" aria-hidden="true" />
        <img class="rambelle-layer" src={rambelleSrc} alt="Rambelle waits behind the RambleDesk workbench" />
      </div>

      <div class="desk-surface" aria-hidden="true">
        <div class="desk-glass"></div>
        <div class="hologram-field">
          <span class="holo-pane holo-main"><i></i><i></i><i></i></span>
          <span class="holo-pane holo-wave"><i></i><i></i><i></i><i></i></span>
          <span class="holo-pane holo-capture"><i></i></span>
          <span class="holo-pane holo-transcript"><i></i><i></i><i></i><i></i></span>
          <span class="holo-pane holo-package"><i></i><i></i></span>
          <span class="holo-pane holo-scan"><i></i><i></i></span>
          <span class="holo-node holo-left"></span>
          <span class="holo-node holo-right"></span>
          <span class="holo-node holo-center"></span>
          <span class="holo-node holo-near"></span>
        </div>
      </div>

      <div class="seal-orbit" aria-hidden="true">
        <span></span>
        <span></span>
        <span></span>
      </div>
    {/if}

    <header class="prototype-nav" aria-label="Prototype navigation">
      <a class="brand" href="/">
        <img src="/assets/rambledesk-app-icon.webp" alt="" />
        <span>RambleDesk</span>
      </a>
      <nav>
        <a href="#happy-path">Happy path</a>
        <a href={releaseUrl}>Release</a>
      </nav>
    </header>

    <div class="copy-zone">
      <p class="eyebrow">{current.eyebrow}</p>
      <h1>
        {#each current.headline as line}
          <span>{line}</span>
        {/each}
      </h1>
      <p class="body-copy">{current.body}</p>
      <div class="hero-actions">
        <a
          class="primary-action"
          href={releaseUrl}
          on:mouseenter={() => wake('download:hover')}
          on:focus={() => wake('download:focus')}
          on:click={() => wake('download:click')}
        >
          {current.primary}
        </a>
        <a
          class="secondary-action"
          href="#happy-path"
          on:mouseenter={() => wake('recording:hover')}
          on:focus={() => wake('recording:focus')}
          on:click={() => wake('recording:click')}
        >
          {current.secondary}
        </a>
      </div>
      <p class="variant-note">{current.note}</p>
    </div>

    <a
      class="scroll-cue"
      href="#happy-path"
      aria-label="Scroll to happy path recording"
      on:mouseenter={() => wake('scroll-cue:hover')}
    >
      <span></span>
    </a>
  </section>

  <section id="happy-path" class="happy-path">
    <div class="recording-copy">
      <span>Below-hero proof</span>
      <h2>Happy path recording lives here.</h2>
      <p>
        The hero stays emotional and low-information. The next section should show a real use path:
        agent request, Ramble, screenshot annotation, feedback package, and continuation.
      </p>
    </div>
    <div class="recording-frame" aria-label="Happy path recording placeholder">
      <div class="recording-top">
        <span></span>
        <span></span>
        <span></span>
      </div>
      <div class="recording-stage">
        <i></i>
        <b></b>
        <strong>Happy path recording</strong>
      </div>
    </div>
  </section>

  {#if !import.meta.env.PROD && showControls}
    <aside class="prototype-switcher" aria-label="Hero prototype variant switcher">
      <button type="button" on:click={() => cycleVariant(-1)} aria-label="Previous variant">←</button>
      <div>
        <strong>{variant} ({current.name})</strong>
        <span>{stateLabel}</span>
      </div>
      <button type="button" on:click={() => cycleVariant(1)} aria-label="Next variant">→</button>
    </aside>
  {/if}
</main>

<style>
  :global(html) {
    background: #eef4f8;
  }

  :global(body) {
    background: #eef4f8;
  }

  .hero-c-prototype {
    min-height: 100svh;
    overflow-x: clip;
    color: #102136;
    background: #eef4f8;
    letter-spacing: 0;
  }

  .hero-screen {
    position: relative;
    min-height: 100svh;
    overflow: hidden;
    isolation: isolate;
    background: #edf4f9;
    --desk-line-left: 71.5%;
    --desk-line-right: 81.3%;
    --character-cut-left: 56%;
    --character-cut-right: 62%;
  }

  .scene-plate,
  .scene-haze,
  .ambient-grid,
  .static-hero-plate,
  .static-copy-wash,
  .command-beam,
  .rambelle-rig,
  .rambelle-layer,
  .rambelle-reflection,
  .desk-surface,
  .seal-orbit {
    position: absolute;
    pointer-events: none;
  }

  .scene-plate {
    inset: -2.4%;
    z-index: 0;
    width: 104.8%;
    height: 104.8%;
    object-fit: cover;
    transform: translate3d(
      calc((var(--px) - 0.5) * -18px),
      calc((var(--py) - 0.5) * -10px),
      0
    ) scale(1.012);
    transition: transform 160ms ease-out;
  }

  .static-hero-plate {
    inset: 0;
    z-index: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: center center;
  }

  .static-copy-wash {
    inset: 0;
    z-index: 1;
    background:
      radial-gradient(circle at 8% 6%, rgb(255 255 255 / 0.86), transparent 24%),
      linear-gradient(90deg, rgb(240 247 252 / 0.96) 0%, rgb(240 247 252 / 0.82) 26%, rgb(240 247 252 / 0.34) 45%, transparent 62%),
      linear-gradient(180deg, rgb(240 247 252 / 0.18) 0%, transparent 44%, rgb(231 241 248 / 0.1) 100%);
  }

  .scene-haze {
    inset: 0;
    z-index: 1;
    background:
      radial-gradient(circle at calc(var(--px) * 100%) calc(var(--py) * 100%), rgb(87 198 192 / calc(0.06 + var(--awake) * 0.08)), transparent 34%),
      linear-gradient(90deg, rgb(241 247 251 / 0.9) 0%, rgb(241 247 251 / 0.5) 34%, transparent 68%),
      linear-gradient(180deg, rgb(244 249 252 / 0.2) 0%, transparent 48%, rgb(232 241 248 / 0.36) 100%);
  }

  .ambient-grid {
    inset: 0;
    z-index: 2;
    opacity: calc(0.18 + var(--awake) * 0.12);
    background-image:
      linear-gradient(rgb(39 117 202 / 0.12) 1px, transparent 1px),
      linear-gradient(90deg, rgb(39 117 202 / 0.11) 1px, transparent 1px);
    background-size: 92px 92px;
    mask-image: linear-gradient(90deg, #000 0%, transparent 62%);
    transform: translate3d(calc((var(--px) - 0.5) * 8px), calc((var(--py) - 0.5) * 4px), 0);
  }

  .rambelle-rig {
    right: clamp(32px, 8vw, 142px);
    bottom: clamp(-220px, -17vh, -92px);
    z-index: 4;
    width: clamp(420px, 45vw, 760px);
    max-height: 106svh;
    transform:
      translate3d(
        calc((var(--px) - 0.5) * -24px),
        calc((var(--py) - 0.5) * -14px),
        0
      )
      scale(calc(1 + var(--awake) * 0.008));
    transform-origin: 52% 78%;
    transition:
      transform 170ms ease-out;
  }

  .rambelle-layer,
  .rambelle-reflection {
    right: 0;
    bottom: 0;
    width: 100%;
    height: auto;
    object-fit: contain;
    transform-origin: 50% var(--character-cut-right);
  }

  .rambelle-layer {
    position: relative;
    z-index: 2;
    display: block;
    filter: drop-shadow(0 30px 56px rgb(49 80 113 / 0.22));
    clip-path: polygon(
      0 0,
      100% 0,
      100% var(--character-cut-right),
      0 var(--character-cut-left)
    );
  }

  .rambelle-reflection {
    top: 0;
    z-index: 1;
    opacity: calc(0.38 + var(--awake) * 0.1);
    filter: blur(5px) saturate(0.66) brightness(0.84) contrast(0.96) drop-shadow(0 0 18px rgb(87 198 192 / 0.18));
    mix-blend-mode: normal;
    clip-path: polygon(
      0 var(--character-cut-left),
      100% var(--character-cut-right),
      100% 100%,
      0 100%
    );
    mask-image: linear-gradient(180deg, transparent 0%, rgb(0 0 0 / 0.82) 38%, rgb(0 0 0 / 0.55) 68%, transparent 92%);
    transform:
      translate3d(1%, 22%, 0)
      scaleY(-0.52)
      skewX(-8deg);
  }

  .desk-surface {
    inset: 0;
    z-index: 5;
    clip-path: polygon(
      0 var(--desk-line-left),
      100% var(--desk-line-right),
      100% 100%,
      0 100%
    );
  }

  .desk-glass {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(ellipse at 68% 82%, rgb(87 198 192 / calc(0.12 + var(--awake) * 0.18)), transparent 30%),
      linear-gradient(102deg, rgb(255 255 255 / 0.01) 0%, rgb(255 255 255 / 0.05) 48%, rgb(111 168 220 / 0.04) 100%),
      linear-gradient(180deg, transparent, rgb(217 233 246 / 0.06));
    backdrop-filter: blur(0.2px) saturate(1.03);
  }

  .desk-glass::before,
  .desk-glass::after {
    position: absolute;
    content: "";
    pointer-events: none;
  }

  .desk-glass::before {
    right: 8%;
    bottom: 8%;
    left: 18%;
    height: 18%;
    background:
      linear-gradient(96deg, transparent 0%, rgb(255 255 255 / 0.34) 23%, transparent 34%),
      linear-gradient(100deg, transparent 46%, rgb(87 198 192 / 0.24) 52%, transparent 60%);
    filter: blur(9px);
    opacity: calc(0.46 + var(--awake) * 0.2);
    transform: rotate(3.8deg);
    transform-origin: left center;
  }

  .desk-glass::after {
    right: 4%;
    bottom: 6%;
    width: 28%;
    height: 1px;
    background: linear-gradient(90deg, transparent, rgb(255 255 255 / 0.86), transparent);
    box-shadow:
      -42vw -4vh 0 rgb(255 255 255 / 0.22),
      -18vw 5vh 0 rgb(87 198 192 / 0.22);
    transform: rotate(3.7deg);
  }

  .desk-surface::before {
    position: absolute;
    top: 74.2%;
    right: -3%;
    left: 18%;
    height: 2px;
    background: linear-gradient(90deg, transparent, rgb(255 255 255 / 0.68), rgb(87 198 192 / 0.36), transparent);
    box-shadow: 0 0 14px rgb(87 198 192 / calc(0.12 + var(--awake) * 0.18));
    content: "";
    transform: rotate(3.7deg);
    transform-origin: left center;
  }

  .desk-surface::after {
    position: absolute;
    inset: 74% 0 0;
    background-image:
      linear-gradient(rgb(87 198 192 / 0.16) 1px, transparent 1px),
      linear-gradient(90deg, rgb(87 198 192 / 0.12) 1px, transparent 1px);
    background-size: 82px 48px;
    opacity: calc(0.2 + var(--awake) * 0.18);
    content: "";
    transform: skewY(4deg) translate3d(calc((var(--px) - 0.5) * 8px), 0, 0);
    transform-origin: left top;
  }

  .hologram-field {
    position: absolute;
    inset: 0;
    opacity: calc(0.94 + var(--awake) * 0.06);
    transform: translate3d(calc((var(--px) - 0.5) * -10px), calc((var(--py) - 0.5) * -5px), 0);
    transition: transform 160ms ease-out;
  }

  .holo-pane,
  .holo-node {
    position: absolute;
    border: 1px solid rgb(87 198 192 / 0.78);
    background: rgb(255 255 255 / 0.12);
    box-shadow:
      0 0 32px rgb(87 198 192 / 0.38),
      inset 0 0 20px rgb(255 255 255 / 0.3);
  }

  .holo-main {
    right: 33%;
    bottom: 9%;
    width: clamp(220px, 22vw, 360px);
    height: clamp(86px, 8vw, 132px);
    border-radius: 8px;
    transform: perspective(740px) rotateX(62deg) rotateZ(-2deg) skewX(-8deg);
  }

  .holo-main i {
    position: absolute;
    border-radius: 999px;
  }

  .holo-main i:nth-child(1) {
    inset: 22% 16%;
    border: 1px solid rgb(111 168 220 / 0.72);
  }

  .holo-main i:nth-child(2) {
    top: 50%;
    left: 22%;
    width: 56%;
    height: 1px;
    background: rgb(87 198 192 / 0.72);
    box-shadow: 0 18px 0 rgb(87 198 192 / 0.42), 0 -18px 0 rgb(87 198 192 / 0.42);
  }

  .holo-main i:nth-child(3) {
    right: 12%;
    bottom: 18%;
    width: 44px;
    height: 44px;
    border: 1px solid rgb(242 160 61 / 0.62);
    clip-path: polygon(25% 5%, 75% 5%, 100% 50%, 75% 95%, 25% 95%, 0 50%);
  }

  .holo-wave {
    right: 21%;
    bottom: 26%;
    width: clamp(112px, 10vw, 170px);
    height: clamp(54px, 5vw, 76px);
    border-radius: 7px;
    transform: perspective(620px) rotateX(55deg) rotateZ(8deg);
  }

  .holo-wave i {
    position: absolute;
    bottom: 18px;
    width: 2px;
    border-radius: 999px;
    background: rgb(87 198 192 / 0.78);
    transform-origin: bottom center;
    animation: wave-pulse 1.6s ease-in-out infinite;
  }

  .holo-wave i:nth-child(1) {
    left: 30%;
    height: 20px;
  }

  .holo-wave i:nth-child(2) {
    left: 42%;
    height: 36px;
    animation-delay: 130ms;
  }

  .holo-wave i:nth-child(3) {
    left: 54%;
    height: 27px;
    animation-delay: 260ms;
  }

  .holo-wave i:nth-child(4) {
    left: 66%;
    height: 42px;
    animation-delay: 390ms;
  }

  .holo-capture {
    right: 52%;
    bottom: 14%;
    width: clamp(130px, 13vw, 220px);
    height: clamp(62px, 6vw, 94px);
    border-radius: 6px;
    transform: perspective(720px) rotateX(64deg) rotateZ(-7deg);
  }

  .holo-capture i {
    position: absolute;
    right: 18%;
    bottom: 24%;
    width: 30%;
    height: 1px;
    background: rgb(87 198 192 / 0.76);
    transform: rotate(-24deg);
  }

  .holo-capture i::after {
    position: absolute;
    right: -2px;
    bottom: -5px;
    width: 12px;
    height: 12px;
    border-top: 2px solid rgb(87 198 192 / 0.7);
    border-right: 2px solid rgb(87 198 192 / 0.7);
    content: "";
    transform: rotate(45deg);
  }

  .holo-node {
    width: 12px;
    height: 12px;
    border-radius: 999px;
  }

  .holo-left {
    right: 59%;
    bottom: 12%;
  }

  .holo-right {
    right: 28%;
    bottom: 15%;
  }

  .holo-center {
    right: 43%;
    bottom: 23%;
  }

  .holo-near {
    right: 18%;
    bottom: 9%;
    width: 15px;
    height: 15px;
  }

  .holo-transcript {
    right: 58%;
    bottom: 7%;
    width: clamp(146px, 14vw, 238px);
    height: clamp(68px, 6.6vw, 108px);
    border-radius: 7px;
    transform: perspective(720px) rotateX(64deg) rotateZ(4deg) skewX(-6deg);
  }

  .holo-transcript i {
    position: absolute;
    left: 14%;
    width: 66%;
    height: 1px;
    background: rgb(87 198 192 / 0.74);
    box-shadow: 0 0 12px rgb(87 198 192 / 0.36);
  }

  .holo-transcript i:nth-child(1) {
    top: 24%;
  }

  .holo-transcript i:nth-child(2) {
    top: 39%;
    width: 50%;
  }

  .holo-transcript i:nth-child(3) {
    top: 55%;
    width: 72%;
  }

  .holo-transcript i:nth-child(4) {
    top: 71%;
    width: 44%;
  }

  .holo-package {
    right: 41%;
    bottom: 27%;
    width: clamp(92px, 9vw, 146px);
    height: clamp(58px, 5.5vw, 86px);
    border-radius: 7px;
    transform: perspective(660px) rotateX(58deg) rotateZ(-14deg);
  }

  .holo-package i {
    position: absolute;
    inset: 18%;
    border: 1px solid rgb(242 160 61 / 0.58);
    clip-path: polygon(25% 5%, 75% 5%, 100% 50%, 75% 95%, 25% 95%, 0 50%);
  }

  .holo-package i:nth-child(2) {
    inset: 35%;
    background: rgb(242 160 61 / 0.16);
  }

  .holo-scan {
    right: 9%;
    bottom: 9%;
    width: clamp(118px, 12vw, 210px);
    height: clamp(54px, 5.4vw, 88px);
    border-radius: 7px;
    transform: perspective(680px) rotateX(61deg) rotateZ(11deg);
  }

  .holo-scan i {
    position: absolute;
    border: 1px solid rgb(111 168 220 / 0.68);
  }

  .holo-scan i:nth-child(1) {
    inset: 22% 16%;
  }

  .holo-scan i:nth-child(2) {
    top: 50%;
    left: 14%;
    width: 72%;
    height: 1px;
    border: 0;
    background: rgb(87 198 192 / 0.78);
    box-shadow:
      0 -18px 0 rgb(87 198 192 / 0.32),
      0 18px 0 rgb(87 198 192 / 0.32);
    animation: scan-slide 2.2s ease-in-out infinite;
  }

  .command-beam {
    inset: 12svh 0 12svh 0;
    z-index: 3;
    width: 100%;
    height: 76svh;
    opacity: calc(0.34 + var(--awake) * 0.58);
  }

  .command-beam path {
    fill: none;
    stroke-linecap: round;
  }

  .beam-soft {
    stroke: rgb(87 198 192 / 0.18);
    stroke-width: 11;
    filter: blur(10px);
  }

  .beam-hot {
    stroke: rgb(111 168 220 / 0.54);
    stroke-width: 2.2;
    stroke-dasharray: 12 18;
    animation: beam-flow 2.8s linear infinite;
  }

  .seal-orbit {
    right: clamp(200px, 28vw, 470px);
    bottom: clamp(175px, 30svh, 330px);
    z-index: 6;
    width: clamp(86px, 8vw, 142px);
    aspect-ratio: 1;
    opacity: calc(0.34 + var(--awake) * 0.58);
    transform: translate3d(calc((var(--px) - 0.5) * -18px), calc((var(--py) - 0.5) * -12px), 0);
  }

  .seal-orbit span {
    position: absolute;
    inset: 0;
    border: 1px solid rgb(87 198 192 / 0.58);
    clip-path: polygon(25% 5%, 75% 5%, 100% 50%, 75% 95%, 25% 95%, 0 50%);
    filter: drop-shadow(0 0 18px rgb(87 198 192 / 0.42));
    animation: seal-pulse 2.6s ease-in-out infinite;
  }

  .seal-orbit span:nth-child(2) {
    inset: 15%;
    animation-delay: 220ms;
  }

  .seal-orbit span:nth-child(3) {
    inset: 30%;
    background: rgb(255 255 255 / 0.46);
    animation-delay: 440ms;
  }

  .prototype-nav {
    position: absolute;
    top: 20px;
    right: 26px;
    left: 26px;
    z-index: 9;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
  }

  .brand,
  .prototype-nav nav,
  .prototype-nav nav a {
    display: inline-flex;
    align-items: center;
  }

  .brand {
    gap: 10px;
    color: #153755;
    font-weight: 760;
  }

  .brand img {
    width: 31px;
    height: 31px;
    border-radius: 7px;
  }

  .prototype-nav nav {
    gap: 8px;
    padding: 6px;
    border: 1px solid rgb(255 255 255 / 0.68);
    border-radius: 8px;
    background: rgb(248 251 253 / 0.44);
    backdrop-filter: blur(18px);
  }

  .prototype-nav nav a {
    min-height: 34px;
    padding: 0 11px;
    border-radius: 7px;
    color: #31516f;
    font-size: 0.86rem;
    font-weight: 670;
  }

  .prototype-nav nav a:hover {
    color: #1e6f95;
    background: rgb(232 242 253 / 0.7);
  }

  .copy-zone {
    position: relative;
    z-index: 8;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    width: min(620px, calc(100vw - 40px));
    padding: clamp(132px, 19svh, 210px) 0 0 clamp(24px, 7vw, 108px);
  }

  .eyebrow {
    max-width: 100%;
    margin: 0;
    color: #1f6f95;
    font-size: 0.82rem;
    font-weight: 760;
    letter-spacing: 0;
    text-transform: uppercase;
  }

  h1 {
    max-width: 11ch;
    margin: 18px 0 0;
    color: #102136;
    font-size: clamp(3.3rem, 7.2vw, 6.7rem);
    line-height: 0.94;
    letter-spacing: 0;
    text-wrap: balance;
  }

  h1 span {
    display: block;
  }

  .body-copy {
    max-width: 570px;
    margin: 25px 0 0;
    color: #405a74;
    font-size: clamp(1rem, 1.45vw, 1.24rem);
    line-height: 1.58;
  }

  .hero-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    margin-top: 30px;
  }

  .primary-action,
  .secondary-action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 46px;
    padding: 0 18px;
    border-radius: 8px;
    font-size: 0.96rem;
    font-weight: 730;
    transition:
      transform 180ms ease,
      box-shadow 180ms ease,
      border-color 180ms ease,
      background 180ms ease;
  }

  .primary-action {
    color: white;
    background: #2775ca;
    box-shadow: 0 18px 38px rgb(39 117 202 / 0.22);
  }

  .primary-action:hover,
  .primary-action:focus-visible {
    transform: translateY(-2px);
    box-shadow: 0 22px 48px rgb(39 117 202 / 0.3);
  }

  .secondary-action {
    color: #1c5279;
    border: 1px solid rgb(111 168 220 / 0.46);
    background: rgb(255 255 255 / 0.56);
    backdrop-filter: blur(18px);
  }

  .secondary-action:hover,
  .secondary-action:focus-visible {
    transform: translateY(-2px);
    border-color: rgb(87 198 192 / 0.72);
    background: rgb(255 255 255 / 0.74);
  }

  .variant-note {
    max-width: 430px;
    margin: 20px 0 0;
    color: rgb(82 103 127 / 0.86);
    font-size: 0.84rem;
    line-height: 1.45;
  }

  .scroll-cue {
    position: absolute;
    bottom: 20px;
    left: 50%;
    z-index: 9;
    width: 34px;
    height: 52px;
    transform: translateX(-50%);
  }

  .scroll-cue span {
    display: block;
    width: 100%;
    height: 100%;
    border: 1px solid rgb(111 168 220 / 0.48);
    border-radius: 999px;
    background: rgb(255 255 255 / 0.32);
    backdrop-filter: blur(12px);
  }

  .scroll-cue span::after {
    display: block;
    width: 5px;
    height: 12px;
    margin: 10px auto 0;
    border-radius: 999px;
    background: #57c6c0;
    content: "";
    animation: scroll-dot 1.9s ease-in-out infinite;
  }

  .variant-b .copy-zone {
    width: min(720px, calc(100vw - 40px));
    padding-top: clamp(116px, 17svh, 180px);
  }

  .variant-b h1 {
    max-width: 13ch;
    font-size: clamp(3.05rem, 6.4vw, 5.9rem);
  }

  .variant-b .body-copy {
    max-width: 610px;
  }

  .variant-b .primary-action {
    background: #102136;
  }

  .variant-b .command-beam {
    opacity: calc(0.5 + var(--awake) * 0.48);
  }

  .variant-c .copy-zone {
    justify-content: flex-end;
    min-height: calc(100svh - 110px);
    padding-top: 92px;
    padding-bottom: clamp(98px, 14svh, 160px);
  }

  .variant-c h1 {
    max-width: 10ch;
    font-size: clamp(3rem, 6.7vw, 6.1rem);
  }

  .variant-c .body-copy,
  .variant-c .variant-note {
    max-width: 520px;
  }

  .variant-c .rambelle-rig {
    right: clamp(14px, 6vw, 110px);
    width: clamp(460px, 48vw, 790px);
  }

  .variant-d .copy-zone {
    justify-content: center;
    min-height: calc(100svh - 90px);
    padding-top: 108px;
    padding-bottom: 78px;
  }

  .variant-d h1 {
    max-width: 10ch;
    font-size: clamp(3.1rem, 6.8vw, 6.25rem);
  }

  .variant-d .body-copy {
    max-width: 520px;
  }

  .variant-d .variant-note {
    display: none;
  }

  .happy-path {
    display: grid;
    grid-template-columns: minmax(260px, 0.74fr) minmax(360px, 1fr);
    gap: clamp(32px, 6vw, 90px);
    align-items: center;
    min-height: 72svh;
    padding: clamp(54px, 8vw, 104px) clamp(22px, 7vw, 112px);
    background: #f7f9fc;
  }

  .recording-copy span {
    color: #1f6f95;
    font-size: 0.82rem;
    font-weight: 760;
    text-transform: uppercase;
  }

  .recording-copy h2 {
    max-width: 10ch;
    margin: 18px 0 0;
    color: #102136;
    font-size: clamp(2.1rem, 4.2vw, 4.2rem);
    line-height: 1;
    letter-spacing: 0;
  }

  .recording-copy p {
    max-width: 560px;
    color: #52677f;
    font-size: 1.02rem;
    line-height: 1.62;
  }

  .recording-frame {
    overflow: hidden;
    border: 1px solid rgb(137 169 203 / 0.36);
    border-radius: 8px;
    background: #eef5fb;
    box-shadow: 0 24px 80px rgb(31 55 83 / 0.14);
  }

  .recording-top {
    display: flex;
    gap: 7px;
    align-items: center;
    height: 38px;
    padding: 0 13px;
    border-bottom: 1px solid rgb(137 169 203 / 0.28);
    background: rgb(255 255 255 / 0.66);
  }

  .recording-top span {
    width: 9px;
    height: 9px;
    border-radius: 999px;
    background: #89a9cb;
  }

  .recording-stage {
    position: relative;
    display: grid;
    place-items: center;
    min-height: min(48vw, 430px);
    background:
      linear-gradient(135deg, rgb(39 117 202 / 0.12), transparent),
      linear-gradient(90deg, rgb(255 255 255 / 0.8), rgb(217 233 246 / 0.42));
  }

  .recording-stage i,
  .recording-stage b {
    position: absolute;
    border: 1px solid rgb(87 198 192 / 0.44);
    border-radius: 8px;
  }

  .recording-stage i {
    width: 62%;
    height: 48%;
  }

  .recording-stage b {
    width: 38%;
    height: 26%;
    transform: translate(30%, 30%);
  }

  .recording-stage strong {
    position: relative;
    color: #31516f;
    font-size: clamp(1.3rem, 2vw, 2rem);
  }

  .prototype-switcher {
    position: fixed;
    right: 50%;
    bottom: 18px;
    z-index: 40;
    display: grid;
    grid-template-columns: 40px minmax(220px, 390px) 40px;
    gap: 8px;
    align-items: center;
    padding: 8px;
    border: 1px solid rgb(15 23 42 / 0.14);
    border-radius: 999px;
    color: #f8fafc;
    background: rgb(7 17 31 / 0.86);
    box-shadow: 0 18px 50px rgb(7 17 31 / 0.24);
    transform: translateX(50%);
    backdrop-filter: blur(18px);
  }

  .prototype-switcher button {
    width: 40px;
    height: 40px;
    border: 0;
    border-radius: 999px;
    color: #f8fafc;
    background: rgb(255 255 255 / 0.12);
    cursor: pointer;
  }

  .prototype-switcher div {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .prototype-switcher strong,
  .prototype-switcher span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .prototype-switcher strong {
    font-size: 0.88rem;
  }

  .prototype-switcher span {
    color: rgb(248 250 252 / 0.72);
    font-size: 0.72rem;
  }

  @keyframes beam-flow {
    to {
      stroke-dashoffset: -60;
    }
  }

  @keyframes seal-pulse {
    0%,
    100% {
      opacity: 0.3;
      transform: scale(0.92) rotate(0deg);
    }

    50% {
      opacity: 1;
      transform: scale(1.04) rotate(8deg);
    }
  }

  @keyframes wave-pulse {
    0%,
    100% {
      opacity: 0.36;
      transform: scaleY(0.72);
    }

    50% {
      opacity: 0.98;
      transform: scaleY(1.1);
    }
  }

  @keyframes scan-slide {
    0%,
    100% {
      opacity: 0.42;
      transform: translateY(-9px);
    }

    50% {
      opacity: 1;
      transform: translateY(9px);
    }
  }

  @keyframes scroll-dot {
    0%,
    100% {
      opacity: 0;
      transform: translateY(0);
    }

    45% {
      opacity: 1;
    }

    80% {
      opacity: 0;
      transform: translateY(18px);
    }
  }

  @media (max-width: 860px) {
    .hero-screen {
      min-height: 100svh;
    }

    .prototype-nav {
      right: 16px;
      left: 16px;
    }

    .prototype-nav nav {
      display: none;
    }

    .rambelle-rig {
      right: -80px;
      bottom: 20svh;
      width: min(560px, 82vw);
      opacity: 0.92;
    }

    .hero-screen {
      --desk-line-left: 70%;
      --desk-line-right: 80%;
      --character-cut-left: 54%;
      --character-cut-right: 60%;
    }

    .desk-surface::before {
      top: 73%;
      left: 0;
      transform: rotate(8deg);
    }

    .holo-main {
      right: 15%;
      bottom: 11%;
      width: 210px;
      height: 76px;
    }

    .holo-wave,
    .holo-capture,
    .holo-transcript,
    .holo-package,
    .holo-scan,
    .holo-node {
      opacity: 0.42;
    }

    .holo-transcript {
      right: 49%;
      bottom: 7%;
      width: 138px;
      height: 62px;
    }

    .holo-package {
      right: 38%;
      bottom: 19%;
    }

    .holo-scan {
      right: 4%;
      bottom: 8%;
      width: 132px;
      height: 58px;
    }

    .copy-zone,
    .variant-b .copy-zone,
    .variant-c .copy-zone,
    .variant-d .copy-zone {
      justify-content: flex-end;
      min-height: 100svh;
      width: calc(100vw - 32px);
      padding: 112px 16px 94px;
    }

    .variant-d .static-hero-plate {
      object-position: 72% center;
    }

    .copy-zone {
      background: linear-gradient(180deg, transparent 0%, rgb(239 246 251 / 0.74) 50%, rgb(239 246 251 / 0.96) 100%);
    }

    .eyebrow {
      font-size: 0.76rem;
    }

    h1,
    .variant-b h1,
    .variant-c h1 {
      max-width: 10ch;
      font-size: clamp(2.7rem, 13vw, 4.3rem);
    }

    .body-copy {
      max-width: 92vw;
      font-size: 1rem;
    }

    .variant-note {
      display: none;
    }

    .happy-path {
      grid-template-columns: 1fr;
    }

    .prototype-switcher {
      grid-template-columns: 38px minmax(150px, 1fr) 38px;
      width: calc(100vw - 24px);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .scene-plate,
    .rambelle-rig,
    .ambient-grid,
    .seal-orbit,
    .hologram-field {
      transform: none;
      transition: none;
    }

    .beam-hot,
    .seal-orbit span,
    .holo-wave i,
    .holo-scan i:nth-child(2),
    .scroll-cue span::after {
      animation: none;
    }
  }
</style>

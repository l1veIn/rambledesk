<script lang="ts">
  import { onMount } from 'svelte'
  import { gsap } from 'gsap'
  import { Mic, Paperclip, ScanLine, Camera, ClipboardList, FileText, Send, Check } from '@lucide/svelte'
  import type { SiteContent } from '../../content/site'

  export let content: SiteContent['lens2']
  export let onsent: () => void = () => {}

  let root: HTMLDivElement
  const shot = content.shots.find((s) => s.id === 'ramble')!

  let tl: gsap.core.Timeline | null = null
  let sealing = false

  export function play() {
    sealing = false
    gsap.set('.seal-overlay', { opacity: 0 })
    gsap.set('.seal-overlay .srow', { opacity: 0, x: -14 })
    gsap.set('.seal-overlay .sdone', { scale: 2.2, opacity: 0 })
    tl?.play(0)
  }

  export function send() {
    if (sealing) return
    sealing = true
    const anim = gsap.timeline({ onComplete: () => onsent() })
    anim
      .to('.seal-overlay', { opacity: 1, duration: 0.15, ease: 'power2.out' }, 0)
      .to('.seal-overlay .srow', { opacity: 1, x: 0, stagger: 0.07, duration: 0.12, ease: 'power2.out' }, 0.1)
      .to(
        '.seal-overlay .sdone',
        { scale: 1, opacity: 1, duration: 0.22, ease: 'power4.out' },
        0.35,
      )
      .to({}, { duration: 0.18 }, 0.62)
  }

  onMount(() => {
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
    if (window.matchMedia('(max-width: 860px)').matches) return

    gsap.set('.seal-overlay', { opacity: 0 })
    gsap.set('.seal-overlay .srow', { opacity: 0, x: -14 })
    gsap.set('.seal-overlay .sdone', { scale: 2.2, opacity: 0 })
    gsap.set('.doc-item', { opacity: 0, y: 44 })

    tl = gsap.timeline({ paused: true })
    tl
      .to('.wb', { y: 0, opacity: 1, scale: 1, duration: 0.4, ease: 'power2.out' }, 0)
      // one message every 200ms, each flowing in from below
      .to(
        '.doc-item',
        { opacity: 1, y: 0, stagger: { each: 0.2, from: 'start' }, duration: 0.38, ease: 'power2.out' },
        0.7,
      )
    // keep the oldest evidence scrolled out after each message lands
    const doc = root.querySelector<HTMLElement>('.editor-doc')
    const scrollDoc = () => doc?.scrollTo({ top: doc.scrollHeight, behavior: 'smooth' })
    shot.feed.forEach((_, i) => tl.add(() => scrollDoc(), 0.72 + i * 0.2 + 0.28))
  })
</script>

<div bind:this={root} class="shot">
  <div class="wb">
    <div class="wb-chrome">
      <span>RambleDesk</span>
      <small>{shot.session}</small>
    </div>
    <div class="wb-cols">
      <div class="wb-left">
        <div class="wb-panel wb-brief">
          <strong>{shot.requestTitle}</strong>
          <p>{shot.requestBody}</p>
        </div>
        <div class="wb-panel wb-editor">
          <div class="editor-head">
            <strong>{shot.draftTitle}</strong>
          </div>
          <div class="editor-doc">
            {#each shot.feed as f}
              {#if f.kind === 'text'}
                <div class="doc-item doc-text">{f.text}</div>
              {:else if f.kind === 'shot'}
                <div class="doc-item doc-shot">
                  <img src="/assets/scene-cryo.webp" alt="" loading="lazy" />
                  <span class="snap-ring"></span>
                  <span class="snap-label"><ScanLine size={12} />{f.label}</span>
                  <span class="snap-note">{f.note}</span>
                </div>
              {:else}
                <div class="doc-item doc-file">
                  <Paperclip size={12} />{f.name}
                </div>
              {/if}
            {/each}
          </div>
        </div>
      </div>
      <div class="wb-right">
        <div class="wb-panel wb-voice">
          <div class="voice-head">
            <Mic size={14} />
            <b>{shot.voiceTitle}</b>
            <span class="rec-chip"><i></i>{shot.voiceState}</span>
          </div>
          <div class="voice-btn">
            <span class="pause-ico"></span>
            <span>{shot.pauseLabel}</span>
          </div>
          <div class="voice-meta">
            <span class="voice-dot"></span>
            <span>{shot.voiceDevice}</span>
          </div>
          <div class="voice-note">{shot.voiceNote} {shot.voicePartial}</div>
          <div class="voice-level" aria-hidden="true"><i></i></div>
        </div>
        <div class="wb-panel wb-context">
          <div class="context-head">+ {shot.addContextLabel}</div>
          <div class="context-chips">
            <span><Camera size={13} />{shot.addContext[0]}</span>
            <span><ClipboardList size={13} />{shot.addContext[1]}</span>
            <span><FileText size={13} />{shot.addContext[2]}</span>
          </div>
        </div>
        <button type="button" class="wb-send" onclick={send}>
          <Send size={14} />
          <span>{shot.sendLabel}</span>
        </button>
        <span class="wb-cancel">{shot.cancelLabel}</span>
      </div>
    </div>

    <div class="seal-overlay" aria-hidden="true">
      <div class="seal-card">
        <strong>{shot.sealTitle}</strong>
        {#each shot.sealFiles as f}
          <span class="srow">{f}</span>
        {/each}
        <div class="sdone" aria-hidden="true">
          <Check size={40} stroke-width={3} />
        </div>
        <small class="shash">{shot.sealHash}</small>
      </div>
    </div>
  </div>
</div>

<style>
  .shot {
    position: relative;
    z-index: 2;
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
  }

  .wb {
    position: relative;
    display: grid;
    grid-template-rows: auto 1fr;
    width: min(1000px, 84vw);
    height: min(62svh, 560px);
    overflow: hidden;
    border: 1px solid rgb(111 168 220 / 28%);
    border-radius: 12px;
    background: linear-gradient(180deg, rgb(17 27 44 / 90%), rgb(10 18 31 / 92%));
    box-shadow:
      0 46px 130px rgb(3 12 24 / 60%),
      inset 0 1px 0 rgb(255 255 255 / 6%);
    backdrop-filter: blur(20px);
  }

  .wb-chrome {
    display: flex;
    gap: 14px;
    align-items: center;
    min-height: 44px;
    padding: 0 16px;
    border-bottom: 1px solid rgb(111 168 220 / 16%);
    color: #cfe3f6;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.82rem;
    font-weight: 800;
  }

  .wb-chrome small {
    margin-left: auto;
    color: rgb(184 212 238 / 58%);
    font-size: 0.74rem;
    font-weight: 600;
  }

  .wb-cols {
    display: grid;
    grid-template-columns: minmax(380px, 1.4fr) minmax(230px, 0.6fr);
    gap: 14px;
    min-height: 0;
    padding: 14px;
  }

  .wb-left,
  .wb-right {
    display: grid;
    gap: 12px;
    min-height: 0;
  }

  .wb-panel {
    padding: 16px;
    border: 1px solid rgb(111 168 220 / 18%);
    border-radius: 10px;
    background: rgb(255 255 255 / 3.5%);
  }

  /* brief: one line "Needs your eyes:" + body */
  .wb-brief strong {
    display: block;
    color: #bffaf6;
    font-size: 0.84rem;
    font-weight: 800;
  }

  .wb-brief p {
    margin: 10px 0 0;
    color: rgb(214 232 249 / 84%);
    font-size: 0.95rem;
    line-height: 1.6;
  }

  /* feedback document */
  .wb-editor {
    display: grid;
    grid-template-rows: auto 1fr;
    gap: 12px;
    min-height: 0;
  }

  .editor-head {
    display: flex;
    align-items: center;
    min-height: 18px;
  }

  .editor-head strong {
    color: #bffaf6;
    font-size: 0.82rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .editor-doc {
    display: flex;
    flex-direction: column;
    gap: 10px;
    height: 178px;
    min-height: 0;
    overflow-y: auto;
    scrollbar-width: none;
  }

  .editor-doc::-webkit-scrollbar {
    display: none;
  }

  .doc-item {
    display: inline-flex;
    gap: 10px;
    align-items: center;
    align-self: start;
  }

  .doc-text {
    max-width: 92%;
    padding: 10px 13px;
    border-radius: 10px 10px 10px 4px;
    color: #eaf5ff;
    background: linear-gradient(135deg, rgb(47 143 214 / 26%), rgb(95 208 201 / 18%));
    font-size: 0.92rem;
    line-height: 1.5;
  }

  .doc-shot {
    position: relative;
    display: block;
    width: min(300px, 88%);
    height: 140px;
    flex: none;
    overflow: hidden;
    border-radius: 10px 10px 10px 4px;
  }

  .doc-shot img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
    filter: saturate(0.9) brightness(1.02);
  }

  .snap-ring {
    position: absolute;
    top: 26%;
    left: 30%;
    width: 44%;
    height: 42%;
    border: 2px solid #f2c77e;
    border-radius: 50%;
    box-shadow: 0 0 16px rgb(242 160 61 / 38%);
  }

  .snap-label {
    position: absolute;
    bottom: 8px;
    right: 8px;
    display: inline-flex;
    gap: 5px;
    align-items: center;
    min-height: 23px;
    padding: 0 8px;
    border-radius: 6px;
    color: #1c2a3c;
    background: rgb(242 199 126 / 92%);
    font-size: 0.7rem;
    font-weight: 800;
  }

  .snap-note {
    position: absolute;
    top: 8px;
    right: 8px;
    min-height: 23px;
    padding: 5px 8px;
    border-radius: 6px;
    color: #e6f2ff;
    background: rgb(13 26 43 / 84%);
    font-size: 0.72rem;
  }

  .doc-file {
    min-height: 32px;
    padding: 7px 11px;
    border: 1px solid rgb(111 168 220 / 26%);
    border-radius: 8px;
    color: #bfdcf4;
    background: rgb(39 117 202 / 10%);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.8rem;
  }

  /* right rail */
  .wb-voice {
    display: grid;
    gap: 11px;
  }

  .voice-head {
    display: flex;
    gap: 9px;
    align-items: center;
    color: #8fd8d2;
  }

  .voice-head b {
    color: #d9f4f1;
    font-size: 0.92rem;
  }

  .rec-chip {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    margin-left: auto;
    min-height: 24px;
    padding: 0 8px;
    border: 1px solid rgb(200 107 123 / 42%);
    border-radius: 6px;
    color: #f3b9c4;
    background: rgb(200 107 123 / 12%);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.7rem;
    font-weight: 800;
  }

  .rec-chip i {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #e4687e;
    animation: rec-blink 1.2s ease infinite;
  }

  .voice-bar,
  .voice-btn {
    display: flex;
    gap: 8px;
    align-items: center;
    justify-content: center;
    min-height: 36px;
    border-radius: 8px;
    color: #d9e8f7;
    background: rgb(255 255 255 / 5%);
    font-size: 0.84rem;
    font-weight: 700;
  }

  .pause-ico {
    position: relative;
    width: 11px;
    height: 13px;
  }

  .pause-ico::before,
  .pause-ico::after {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 3px;
    border-radius: 1px;
    background: #d9e8f7;
    content: '';
  }

  .pause-ico::before {
    left: 1px;
  }

  .pause-ico::after {
    right: 1px;
  }

  .voice-meta {
    display: flex;
    gap: 8px;
    align-items: center;
    color: rgb(184 212 238 / 62%);
    font-size: 0.8rem;
  }

  .voice-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #e4687e;
  }

  .voice-note {
    color: rgb(184 212 238 / 74%);
    font-size: 0.8rem;
    line-height: 1.5;
  }

  .voice-level {
    height: 4px;
    overflow: hidden;
    border-radius: 2px;
    background: rgb(255 255 255 / 7%);
  }

  .voice-level i {
    display: block;
    width: 72%;
    height: 100%;
    border-radius: 2px;
    background: linear-gradient(90deg, #57c6c0, #2775ca);
    animation: level-flow 2.4s ease-in-out infinite alternate;
  }

  .wb-context {
    display: grid;
    gap: 11px;
  }

  .context-head {
    color: #a8cdf2;
    font-size: 0.82rem;
    font-weight: 700;
  }

  .context-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .context-chips span {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    min-height: 30px;
    padding: 6px 10px;
    border: 1px solid rgb(111 168 220 / 22%);
    border-radius: 7px;
    color: #a8cdf2;
    background: rgb(39 117 202 / 12%);
    font-size: 0.78rem;
    font-weight: 700;
  }

  .wb-send {
    display: inline-flex;
    gap: 8px;
    align-items: center;
    justify-content: center;
    min-height: 36px;
    border: 0;
    border-radius: 9px;
    color: #071523;
    background: linear-gradient(135deg, #5fd0c9, #2f8fd6);
    box-shadow: 0 12px 28px rgb(39 117 202 / 28%);
    font-size: 0.92rem;
    font-weight: 800;
    cursor: pointer;
    transition:
      transform 160ms ease,
      box-shadow 160ms ease;
  }

  .wb-send:hover {
    transform: translateY(-1px);
    box-shadow: 0 16px 34px rgb(39 117 202 / 36%);
  }

  .wb-send:active {
    transform: translateY(0);
  }

  .wb-cancel {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 36px;
    border: 1px solid rgb(200 107 123 / 32%);
    border-radius: 9px;
    color: rgb(243 185 196 / 82%);
    font-size: 0.82rem;
    font-weight: 700;
  }

  @keyframes level-flow {
    from {
      width: 46%;
    }
    to {
      width: 84%;
    }
  }

  /* sealing overlay: send → evidence gets packed */
  .seal-overlay {
    position: absolute;
    inset: 0;
    z-index: 5;
    display: grid;
    place-items: center;
    background: rgb(5 13 24 / 62%);
    backdrop-filter: blur(6px);
    opacity: 0;
    pointer-events: none;
  }

  .seal-card {
    position: relative;
    display: grid;
    gap: 9px;
    width: min(430px, 86%);
    padding: 22px 24px;
    border: 1px solid rgb(87 198 192 / 34%);
    border-radius: 12px;
    background: linear-gradient(180deg, rgb(19 33 52 / 92%), rgb(9 20 34 / 94%));
    box-shadow:
      0 40px 110px rgb(3 12 24 / 62%),
      0 0 60px rgb(87 198 192 / 14%);
  }

  .seal-card strong {
    color: #cdf7f3;
    font-size: 0.9rem;
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }

  .srow {
    display: flex;
    align-items: center;
    min-height: 34px;
    padding: 0 12px;
    border: 1px solid rgb(111 168 220 / 20%);
    border-radius: 7px;
    color: #d9ecfb;
    background: rgb(255 255 255 / 3%);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.86rem;
  }

  .sdone {
    position: absolute;
    top: 40%;
    right: 12%;
    display: grid;
    place-items: center;
    width: 62px;
    height: 62px;
    border-radius: 50%;
    color: #071523;
    background: linear-gradient(135deg, #5fd0c9, #2f8fd6);
    box-shadow:
      0 0 0 8px rgb(95 208 201 / 14%),
      0 0 30px rgb(87 198 192 / 45%);
  }

  .shash {
    color: rgb(184 212 238 / 70%);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.78rem;
  }

  @keyframes rec-blink {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.25;
    }
  }

  @media (max-width: 860px) {
    .shot {
      position: relative;
      height: auto;
    }

    .wb {
      width: 100%;
      height: auto;
      min-height: 0;
    }

    .wb-cols {
      grid-template-columns: 1fr;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .shot {
      position: relative;
      height: auto;
    }

    .wb {
      width: 100%;
      height: auto;
      min-height: 0;
    }

    .wb-cols {
      grid-template-columns: 1fr;
    }

    .seal-overlay {
      display: none;
    }
  }
</style>

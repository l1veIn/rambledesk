<script lang="ts">
  import { onMount } from 'svelte'
  import { gsap } from 'gsap'
  import { Mic, Camera, ClipboardList, FileText, Send, Check } from '@lucide/svelte'
  import MessageFeed from './MessageFeed.svelte'
  import type { SiteContent } from '../../content/site'

  export let content: SiteContent['lens2']
  export let onsent: () => void = () => {}

  let root: HTMLDivElement
  const shot = content.shots.find((s) => s.id === 'ramble')!

  let tl: gsap.core.Timeline | null = null
  let sealing = false
  let feedRef: MessageFeed

  export function play() {
    sealing = false
    gsap.set('.seal-overlay', { opacity: 0 })
    gsap.set('.seal-overlay .srow', { opacity: 0, x: -14 })
    gsap.set('.seal-overlay .sdone', { scale: 2.2, opacity: 0 })
    feedRef?.play()
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

    tl = gsap.timeline({ paused: true })
    tl.to('.wb', { y: 0, opacity: 1, scale: 1, duration: 0.4, ease: 'power2.out' }, 0)
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
          <MessageFeed bind:this={feedRef} messages={shot.feed} />
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

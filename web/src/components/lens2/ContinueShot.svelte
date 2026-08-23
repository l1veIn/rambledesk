<script lang="ts">
  import { onMount } from 'svelte'
  import { gsap } from 'gsap'
  import type { SiteContent } from '../../content/site'

  export let content: SiteContent['lens2']

  let root: HTMLDivElement
  const shot = content.shots.find((s) => s.id === 'continue')!
  const ask = content.shots.find((s) => s.id === 'ask')!

  let tl: gsap.core.Timeline | null = null

  export function play() {
    tl?.play(0)
  }

  onMount(() => {
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
    if (window.matchMedia('(max-width: 860px)').matches) return

    const splitChars = (el: HTMLElement, text: string) => {
      el.innerHTML = ''
      text.split(/(\s+)/).forEach((token) => {
        if (!token) return
        if (/^\s+$/.test(token)) {
          el.appendChild(document.createTextNode(' '))
          return
        }
        const word = document.createElement('span')
        word.style.whiteSpace = 'nowrap'
        token.split('').forEach((c) => {
          const s = document.createElement('span')
          s.className = 'ch'
          s.textContent = c
          word.appendChild(s)
        })
        el.appendChild(word)
      })
    }
    splitChars(root.querySelector<HTMLElement>('.cmd2 .cmdtext')!, shot.prompt2)

    gsap.set('.cmd2 .ch', { opacity: 0 })
    gsap.set('.tui-tool2', { opacity: 0, y: 10 })
    gsap.set('.tui-line', { clipPath: 'inset(0 100% 0 0)', opacity: 0.25 })
    gsap.set('.lens2-outro', { opacity: 0 })

    tl = gsap.timeline({ paused: true })
    tl
      .to('.tui', { y: 0, opacity: 1, scale: 1, duration: 0.4, ease: 'power2.out' }, 0)
      .to('.cmd2 .ch', { opacity: 1, stagger: 0.03, duration: 0.028 }, 0.5)
      .to('.tui-tool2', { opacity: 1, y: 0, duration: 0.3 }, 1.35)
      .to('.tui-line', { clipPath: 'inset(0 0% 0 0)', opacity: 1, stagger: 0.3, duration: 0.3 }, 1.75)
      .to('.lens2-outro', { opacity: 1, duration: 0.45, ease: 'power2.out' }, 2.45)
  })
</script>

<div bind:this={root} class="shot">
  <div class="tui">
    <div class="tui-history">
      <div class="tui-cmdline">
        <span class="tui-arrow">❯</span>
        <span class="cmdtext">{ask.prompt}</span>
      </div>
      <div class="tui-log dim">{ask.logs[0]}</div>
      <div class="tui-tool">
        <span class="tool-dot"></span>
        <span>{ask.tool}</span>
      </div>
      <div class="tui-cmdline cmd2">
        <span class="tui-arrow">❯</span>
        <span class="cmdtext">{shot.prompt2}</span>
      </div>
      <div class="tui-tool2">
        <span class="tool-dot"></span>
        <span>{shot.tool2}</span>
      </div>
      {#each shot.lines as line}
        <div class="tui-line">{line}</div>
      {/each}
    </div>
    <div class="tui-hr"></div>
    <div class="tui-idle"><span class="tui-arrow dim">❯</span><span class="tui-caret"></span></div>
  </div>
  <div class="lens2-outro">
    <span>{content.tagline}</span>
    <small>{shot.sub}</small>
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

  .tui {
    display: flex;
    flex-direction: column;
    width: min(920px, 88vw);
    min-height: 380px;
    padding: 30px;
    border: 1px solid rgb(137 169 203 / 16%);
    border-radius: 12px;
    color: #e6e6e6;
    background: linear-gradient(180deg, rgba(4, 8, 13, 0.96), rgba(2, 5, 9, 0.98));
    box-shadow:
      0 50px 140px rgb(0 0 0 / 66%),
      inset 0 1px 0 rgb(255 255 255 / 5%);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, 'Cascadia Mono', monospace;
  }

  .tui-history {
    display: grid;
    gap: 12px;
    align-content: start;
    min-height: 240px;
  }

  .tui-cmdline {
    display: flex;
    gap: 14px;
    align-items: baseline;
  }

  .tui-arrow {
    color: #d97d6c;
    font-weight: 800;
    font-size: 1.2rem;
    transform: translateY(1px);
  }

  .tui-arrow.dim {
    color: rgb(217 125 108 / 62%);
  }

  .cmdtext {
    display: inline-block;
    color: #f2f2f2;
    font-size: 1.06rem;
    line-height: 1.6;
  }

  :global(.ch) {
    display: inline-block;
    will-change: opacity;
  }

  .tui-log,
  .tui-line {
    color: #d9d9d9;
    font-size: 0.97rem;
    line-height: 1.5;
  }

  .tui-log.dim {
    color: rgb(217 217 217 / 62%);
  }

  .tui-tool,
  .tui-tool2 {
    display: flex;
    gap: 10px;
    align-items: center;
    margin-top: 6px;
    padding: 9px 12px;
    border: 1px solid rgb(74 222 128 / 22%);
    border-radius: 6px;
    color: #d6f5e0;
    background: rgb(74 222 128 / 7%);
    font-size: 0.95rem;
    font-weight: 700;
    text-shadow: 0 0 16px rgb(74 222 128 / 35%);
  }

  .tool-dot {
    width: 9px;
    height: 9px;
    flex: none;
    border-radius: 50%;
    background: #4ade80;
    box-shadow: 0 0 10px rgb(74 222 128 / 80%);
  }

  .tui-hr {
    height: 1px;
    margin: 20px 0;
    background: linear-gradient(90deg, rgb(255 255 255 / 22%), rgb(255 255 255 / 8%) 70%, transparent);
  }

  .tui-idle {
    display: flex;
    gap: 14px;
    align-items: baseline;
    min-height: 34px;
  }

  .tui-caret {
    display: inline-block;
    width: 10px;
    height: 20px;
    background: rgb(230 230 230 / 82%);
    animation: caret-blink 1.1s steps(1) infinite;
  }

  .lens2-outro {
    position: absolute;
    bottom: 8vh;
    left: 50%;
    z-index: 3;
    display: grid;
    gap: 8px;
    width: min(560px, 84vw);
    text-align: center;
    transform: translateX(-50%);
  }

  .lens2-outro span {
    color: #f2c77e;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.92rem;
    font-weight: 800;
    letter-spacing: 0.22em;
    text-transform: uppercase;
  }

  .lens2-outro small {
    color: rgb(214 232 249 / 66%);
    font-size: 0.92rem;
    line-height: 1.6;
  }

  @keyframes caret-blink {
    0%,
    49% {
      opacity: 1;
    }
    50%,
    100% {
      opacity: 0;
    }
  }

  @media (max-width: 860px) {
    .shot {
      position: relative;
      height: auto;
    }

    .tui {
      min-height: 0;
      padding: 24px;
    }

    .tui-history {
      min-height: 0;
    }

    .lens2-outro {
      position: relative;
      bottom: auto;
      left: auto;
      width: 100%;
      transform: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .shot {
      position: relative;
      height: auto;
    }

    .tui {
      min-height: 0;
      padding: 24px;
    }

    .tui-history {
      min-height: 0;
    }

    .lens2-outro {
      position: relative;
      bottom: auto;
      left: auto;
      width: 100%;
      transform: none;
    }
  }
</style>

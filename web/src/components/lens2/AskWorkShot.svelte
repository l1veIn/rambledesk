<script lang="ts">
  import { onMount } from 'svelte'
  import { gsap } from 'gsap'
  import type { SiteContent } from '../../content/site'

  export let content: SiteContent['lens2']

  let root: HTMLDivElement

  const shot = content.shots.find((s) => s.id === 'ask')!
  const byId = (id: string) => content.shots.find((s) => s.id === id)!

  let tl: gsap.core.Timeline | null = null

  function splitChars(el: HTMLElement, text: string) {
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

  export function play() {
    if (!tl) return
    // reset the typed line back into the input before replaying
    // (the line was moved into history at the end of the last run)
    const input = root.querySelector<HTMLElement>('.tui-input')
    const line = root.querySelector<HTMLElement>('.tui-cmdline')
    const el = line?.querySelector<HTMLElement>('.cmdtext')
    if (input && line && el) {
      line.removeChild(el)
      input.appendChild(el)
      splitChars(el, shot.prompt)
    }
    tl.play(0)
  }

  onMount(() => {
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
    if (window.matchMedia('(max-width: 860px)').matches) return

    splitChars(root.querySelector<HTMLElement>('.cmd-typing')!, shot.prompt)

    gsap.set('.cmd-typing .ch', { opacity: 0 })
    gsap.set('.tui-cmdline', { opacity: 0 })
    gsap.set('.tui-log', { clipPath: 'inset(0 100% 0 0)', opacity: 0.25 })
    gsap.set('.tui-tool', { opacity: 0, y: 10 })
    gsap.set('.tui-tool-out', { opacity: 0 })

    tl = gsap.timeline({ paused: true })
    tl
      .to('.tui', { y: 0, opacity: 1, scale: 1, duration: 0.4, ease: 'power2.out' }, 0)
      .to('.cmd-typing .ch', { opacity: 1, stagger: 0.026, duration: 0.025 }, 0.45)
      .to('.cmd-typing', { y: -52, opacity: 0, duration: 0.3, ease: 'power2.in' }, 1.85)
      .add(
        () => {
          const input = root.querySelector<HTMLElement>('.tui-input')
          const line = root.querySelector<HTMLElement>('.tui-cmdline')
          const el = input?.querySelector<HTMLElement>('.cmdtext')
          if (line && el) {
            line.appendChild(el)
            gsap.set(el, { y: 0, opacity: 1 })
            gsap.to(line, { opacity: 1, duration: 0.25 })
          }
        },
        2.15,
      )
      .to('.tui-log', { clipPath: 'inset(0 0% 0 0)', opacity: 1, stagger: 0.22, duration: 0.24 }, 2.25)
      .to('.tui-tool', { opacity: 1, y: 0, duration: 0.3 }, 3.25)
      .to('.tui-tool-out', { opacity: 1, duration: 0.25 }, 3.55)
  })
</script>

<div bind:this={root} class="shot">
  <div class="tui">
    <div class="tui-history">
      <div class="tui-cmdline">
        <span class="tui-arrow">❯</span>
      </div>
      {#each shot.logs as log, i}
        <div class="tui-log {i === 0 ? '' : 'dim'}">{log}</div>
      {/each}
      <div class="tui-tool">
        <span class="tool-dot"></span>
        <span>{shot.tool}</span>
      </div>
      <div class="tui-tool-out">└ {shot.toolOut}</div>
    </div>
    <div class="tui-hr"></div>
    <div class="tui-input">
      <span class="tui-arrow">❯</span>
      <span class="cmdtext cmd-typing">{shot.prompt}</span>
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

  .tui-cmdline,
  .tui-input {
    display: flex;
    gap: 14px;
    align-items: baseline;
  }

  .tui-input {
    min-height: 40px;
  }

  .tui-arrow {
    color: #d97d6c;
    font-weight: 800;
    font-size: 1.2rem;
    transform: translateY(1px);
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

  .tui-log {
    color: #d9d9d9;
    font-size: 0.98rem;
    line-height: 1.5;
  }

  .tui-log.dim {
    color: rgb(217 217 217 / 62%);
  }

  .tui-tool {
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

  .tui-tool-out {
    padding-left: 24px;
    color: rgb(217 217 217 / 55%);
    font-size: 0.9rem;
  }

  .tui-hr {
    height: 1px;
    margin: 20px 0;
    background: linear-gradient(90deg, rgb(255 255 255 / 22%), rgb(255 255 255 / 8%) 70%, transparent);
  }

  @media (max-width: 860px) {
    .tui {
      min-height: 0;
      padding: 24px;
    }

    .tui-history {
      min-height: 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .tui {
      min-height: 0;
      padding: 24px;
    }

    .tui-history {
      min-height: 0;
    }
  }
</style>

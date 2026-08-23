<script lang="ts">
  import { onDestroy, tick } from 'svelte'
  import { gsap } from 'gsap'
  import { Paperclip, ScanLine } from '@lucide/svelte'
  import type { SiteContent } from '../../content/site'

  type FeedMessage = SiteContent['lens2']['shots'][number] extends infer T
    ? T extends { feed: infer F }
      ? F[number]
      : never
    : never

  export let messages: FeedMessage[]
  export let itemKey = 'feed'

  let box: HTMLDivElement
  let shown: FeedMessage[] = []
  let timers: number[] = []

  export function play() {
    // fresh chat: empty the box, then push every message within ~2s
    timers.forEach((t) => window.clearTimeout(t))
    timers = []
    shown = []
    if (box) box.scrollTop = 0

    const gaps = randGaps(messages.length - 1, 2000)
    let at = 60
    messages.forEach((msg, i) => {
      timers.push(
        window.setTimeout(() => push(msg), at),
      )
      at += i < gaps.length ? gaps[i] : 0
    })
  }

  function randGaps(count: number, total: number) {
    if (count <= 0) return []
    // uniform gaps scaled to fit exactly `total` (with a floor of 220ms)
    const floor = 220
    const raw = Array.from({ length: count }, () => Math.random())
    const sum = raw.reduce((a, b) => a + b, 0)
    let gaps = raw.map((r) => 220 + ((total - floor * count) * r) / sum)
    const drift = total - gaps.reduce((a, b) => a + b, 0)
    gaps[0] = Math.max(150, gaps[0] + drift)
    return gaps
  }

  async function push(msg: FeedMessage) {
    shown = [...shown, msg]
    await tick()
    const el = box?.children[shown.length - 1] as HTMLElement | undefined
    if (el) {
      gsap.from(el, { y: 44, opacity: 0, duration: 0.38, ease: 'power2.out' })
    }
    box?.scrollTo({ top: box.scrollHeight, behavior: 'smooth' })
  }

  onDestroy(() => timers.forEach((t) => window.clearTimeout(t)))
</script>

<div class="feed" bind:this={box} aria-label="Feedback feed">
  {#each shown as msg}
    {#if msg.kind === 'text'}
      <div class="msg msg-text">{msg.text}</div>
    {:else if msg.kind === 'shot'}
      <div class="msg msg-shot">
        <img src="/assets/scene-cryo.webp" alt="" loading="lazy" />
        <span class="snap-ring"></span>
        <span class="snap-badge"><ScanLine size={12} />{msg.label}</span>
      </div>
    {:else}
      <div class="msg msg-file">
        <Paperclip size={12} />{msg.name}
      </div>
    {/if}
  {/each}
</div>

<style>
  .feed {
    display: flex;
    flex-direction: column;
    gap: 10px;
    height: 178px;
    min-height: 0;
    overflow-y: auto;
    scrollbar-width: none;
  }

  .feed::-webkit-scrollbar {
    display: none;
  }

  .msg {
    position: relative;
    flex: none;
  }

  .msg-text {
    max-width: 92%;
    padding: 10px 13px;
    border-radius: 10px 10px 10px 4px;
    color: #eaf5ff;
    background: linear-gradient(135deg, rgb(47 143 214 / 26%), rgb(95 208 201 / 18%));
    font-size: 0.92rem;
    line-height: 1.5;
  }

  .msg-shot {
    display: block;
    width: min(300px, 88%);
    height: 140px;
    overflow: hidden;
    border-radius: 10px 10px 10px 4px;
  }

  .msg-shot img {
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

  .snap-badge {
    position: absolute;
    right: 8px;
    bottom: 8px;
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

  .msg-file {
    min-height: 32px;
    padding: 7px 11px;
    border: 1px solid rgb(111 168 220 / 26%);
    border-radius: 8px;
    color: #bfdcf4;
    background: rgb(39 117 202 / 10%);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.8rem;
  }
</style>

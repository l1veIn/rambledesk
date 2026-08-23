<script lang="ts">
  export let tags: string[]
  export let index = 0
  export let onjump: (i: number) => void = () => {}
</script>

<nav class="lens-nav" aria-label="Lens navigation">
  <span class="lens-nav-tags" aria-hidden="true">{index + 1} / {tags.length} · {tags[index]}</span>
  <div class="lens-nav-rail">
    {#each tags as tag, i}
      <button
        type="button"
        class:active={i === index}
        aria-label={`Go to ${tag}`}
        aria-current={i === index ? 'true' : undefined}
        onclick={() => onjump(i)}
      >
        <i></i>
      </button>
    {/each}
  </div>
</nav>

<style>
  .lens-nav {
    position: absolute;
    bottom: 22px;
    left: 50%;
    z-index: 6;
    display: grid;
    gap: 8px;
    justify-items: center;
    transform: translateX(-50%);
    pointer-events: none;
  }

  .lens-nav-tags {
    color: rgb(184 212 238 / 64%);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }

  .lens-nav-rail {
    display: flex;
    gap: 8px;
    pointer-events: auto;
  }

  button {
    position: relative;
    display: grid;
    place-items: center;
    padding: 6px 2px;
    border: 0;
    background: transparent;
    cursor: pointer;
  }

  button i {
    display: block;
    width: 22px;
    height: 3px;
    border-radius: 2px;
    background: rgb(137 169 203 / 34%);
    transition:
      width 200ms ease,
      background 200ms ease,
      box-shadow 200ms ease;
  }

  button:hover i {
    background: rgb(137 169 203 / 62%);
  }

  button.active i {
    width: 40px;
    background: linear-gradient(90deg, #57c6c0, #2775ca);
    box-shadow: 0 0 12px rgb(87 198 192 / 38%);
  }

  @media (max-width: 860px) {
    .lens-nav {
      display: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .lens-nav {
      display: none;
    }
  }
</style>

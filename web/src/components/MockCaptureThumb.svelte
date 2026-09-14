<script lang="ts">
  // Illustrative capture used inside the mock: it stands in for a screenshot the human
  // attached. It is drawn in CSS on purpose, so the site never presents it as a real capture.
  let {
    settings,
    items,
    issue,
    alt,
    compact = false,
  }: { settings: string; items: string[]; issue: string; alt: string; compact?: boolean } = $props()
</script>

<div class="capture-thumb" class:compact role="img" aria-label={alt}>
  <div class="paper">
    <div class="heading">
      <svg width="13" height="13" viewBox="0 0 24 24" aria-hidden="true">
        <path d="m9 3-1 3-3 1v4l2 1-2 1v4l3 1 1 3h6l1-3 3-1v-4l-2-1 2-1V7l-3-1-1-3Z" />
        <circle cx="12" cy="12" r="3" />
      </svg>
      <span>{settings}</span>
    </div>
    <div class="list">
      {#each items as item, index}
        <span class:obscured={index === items.length - 1}><i></i>{item}</span>
      {/each}
    </div>
    <div class="scrollbar"><span></span></div>
  </div>
  {#if !compact}<span class="issue">{issue}</span>{/if}
</div>

<style>
  .capture-thumb {
    min-width: 0;
  }
  .paper {
    overflow: hidden;
    border: 1px solid #dbe4ec;
    border-radius: 5px;
    background: #f8fbfd;
    box-shadow: 0 6px 16px #1c3a5620;
  }
  .heading {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 9px 10px 7px;
    border-bottom: 1px solid #e6edf3;
    color: #35566e;
    font-size: 11px;
    font-weight: 650;
  }
  .heading svg {
    flex: 0 0 auto;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .list {
    display: grid;
    padding: 3px 7px 0;
  }
  .list > span {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 3px;
    border-bottom: 1px solid #eef3f7;
    color: #5b7285;
    font-size: 10px;
    line-height: 1.4;
  }
  .list > span:last-child {
    border-bottom: 0;
  }
  .list i {
    width: 4px;
    height: 4px;
    flex: 0 0 auto;
    border: 1px solid #9db1c2;
    border-radius: 1px;
  }
  .list > span.obscured {
    color: #75899a;
    mask-image: linear-gradient(#000 25%, transparent 92%);
  }
  .scrollbar {
    margin-top: 2px;
    padding: 3px 4px;
    border-top: 1px solid #d9b877;
    background: #f6ecd8;
  }
  .scrollbar > span {
    display: block;
    width: 62%;
    height: 3px;
    border-radius: 3px;
    background: #b8934f;
  }
  .issue {
    display: block;
    margin-top: 7px;
    color: #b98a3c;
    font-size: 10px;
    font-weight: 600;
    line-height: 1.5;
  }
  .compact .heading {
    padding: 6px 8px 5px;
    font-size: 9px;
  }
  .compact .list {
    padding: 2px 5px 0;
  }
  .compact .list > span {
    padding: 3px 2px;
    font-size: 8px;
  }
  .compact .scrollbar {
    padding: 2px 3px;
  }
  .compact .scrollbar > span {
    height: 2px;
  }
</style>

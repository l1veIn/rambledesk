<script lang="ts">
  import type { ReviewAnnotation } from '../../generated/feedback'
  import { useWorkbenchCapabilities } from '../../capabilities/capabilityContext'
  import type { ReviewDocument, ReviewNode } from './reviewMarkdown'
  import ReviewMarkdownNode from './ReviewMarkdownNode.svelte'
  export let document: ReviewDocument
  export let nodes: ReviewNode[] | undefined = undefined
  export let annotations: ReviewAnnotation[]
  export let selectedId: string | null
  export let onSelect: (id: string) => void
  const capabilities = useWorkbenchCapabilities()
  const openLink = (url: string) => { void capabilities.externalLinks.implementation.open(url).catch((cause) => console.warn('Could not open external URL', cause)) }
</script>

<div data-review-text class="review-markdown">
  {#each nodes ?? document.nodes as node}<ReviewMarkdownNode {node} {annotations} {selectedId} {onSelect} onOpenLink={openLink} />{/each}
</div>

<style>
  .review-markdown { overflow-wrap: anywhere; font-size: 15px; line-height: 1.85; }
  .review-markdown :global(p) { margin: .8em 0; }
  .review-markdown :global(h1), .review-markdown :global(h2), .review-markdown :global(h3), .review-markdown :global(h4), .review-markdown :global(h5), .review-markdown :global(h6) { margin: 1.2em 0 .55em; font-weight: 650; line-height: 1.4; }
  .review-markdown :global(h1) { font-size: 1.75em; }
  .review-markdown :global(h2) { font-size: 1.4em; }
  .review-markdown :global(h3) { font-size: 1.2em; }
  .review-markdown :global(ul), .review-markdown :global(ol) { margin: .7em 0; padding-left: 1.6em; }
  .review-markdown :global(ul) { list-style-type: disc; }
  .review-markdown :global(ol) { list-style-type: decimal; }
  .review-markdown :global(li > p) { margin: .15em 0; }
  .review-markdown :global(blockquote) { margin: 1em 0; padding-left: 1em; border-left: 3px solid var(--border); color: var(--muted-foreground); }
  .review-markdown :global(pre) { overflow: auto; padding: 1em; border: 1px solid var(--border); border-radius: .5em; background: var(--muted); white-space: pre; }
  .review-markdown :global(code) { font-family: var(--font-mono); font-size: .9em; background: var(--muted); white-space: pre-wrap; }
  .review-markdown :global(pre code) { white-space: pre; }
  .review-markdown :global(table) { display: block; overflow: auto; border-collapse: collapse; margin: 1em 0; max-width: 100%; }
  .review-markdown :global(th), .review-markdown :global(td) { border: 1px solid var(--border); padding: .45em .8em; text-align: left; }
  .review-markdown :global(th) { background: var(--muted); }
  .review-markdown :global(a[href]) { color: var(--primary); text-decoration: underline; cursor: pointer; }
  .review-markdown :global(hr) { margin: 1.5em 0; border-color: var(--border); }
  .review-markdown :global(input[type=checkbox]) { margin-right: .5em; }
  .review-markdown :global(.review-image-placeholder) { display: inline-block; padding: .5em; border: 1px dashed var(--border); color: var(--muted-foreground); }
  .review-markdown :global([data-source-fallback]) { white-space: pre-wrap; font-family: var(--font-mono); }
  .review-markdown :global([data-annotation-ids]) { cursor: pointer; background: color-mix(in srgb, #fbbf24 10%, transparent); border-bottom: 1px solid color-mix(in srgb, #d99a16 55%, transparent); }
  .review-markdown :global(.review-selected) { background: color-mix(in srgb, var(--primary) 20%, transparent); border-color: var(--primary); }
  .review-markdown :global(.review-deletion) { text-decoration: line-through; }
  .review-markdown :global([data-annotation-ids]:focus-visible) { outline: 2px solid var(--ring); }
</style>

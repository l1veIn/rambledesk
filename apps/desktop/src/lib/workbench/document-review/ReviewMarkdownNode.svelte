<script lang="ts">
  import type { ReviewAnnotation } from '../../generated/feedback'
  import { isSafeHttpUrl } from '../../editor/linkify'
  import { highlightedLeaf, type ReviewNode } from './reviewMarkdown'
  export let node: ReviewNode
  export let annotations: ReviewAnnotation[]
  export let selectedId: string | null
  export let onSelect: (id: string) => void
  export let onOpenLink: (url: string) => void
  $: segments = node.leaf ? highlightedLeaf(node.leaf, annotations) : []
  $: safeLink = node.href && isSafeHttpUrl(node.href) ? node.href : undefined
  function activate(event: MouseEvent, ids: string[]) {
    event.preventDefault()
    event.stopPropagation()
    if (!document.getSelection()?.toString()) onSelect(ids.includes(selectedId ?? '') ? selectedId! : ids[0])
  }
</script>

{#if node.tag === 'br'}<br />
{:else if node.tag === 'hr'}<hr />
{:else if node.tag === 'image'}<span data-review-unmapped class="review-image-placeholder" aria-label={node.placeholder}>▧ {node.placeholder}</span>
{:else if node.tag === 'a'}
  <a href={safeLink} onclick={(event) => { event.preventDefault(); if (safeLink && !document.getSelection()?.toString()) onOpenLink(safeLink) }}>{#each node.children as child}<svelte:self node={child} {annotations} {selectedId} {onSelect} {onOpenLink} />{/each}</a>
{:else if node.leaf}
  <svelte:element this={node.tag} data-review-leaf={node.leaf.id} data-source-fallback={node.leaf.fallback || undefined}>{#each segments as segment}{#if segment.annotationIds.length}<span role="button" tabindex="0"
        data-annotation-ids={segment.annotationIds.join(' ')} class:review-selected={segment.annotationIds.includes(selectedId ?? '')}
        class:review-deletion={annotations.some((note) => segment.annotationIds.includes(note.id) && note.kind === 'suggestion' && note.replacement === '')}
        onclick={(event) => activate(event, segment.annotationIds)} onkeydown={(event) => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); event.stopPropagation(); onSelect(segment.annotationIds[0]) } }}>{segment.text}</span>{:else}{segment.text}{/if}{/each}</svelte:element>
{:else}
  <svelte:element this={node.tag} {...node.tag === 'ol' ? { start: node.start ?? undefined } : {}}>{#if node.checked !== undefined && node.checked !== null}<input type="checkbox" checked={node.checked} disabled aria-label="Task" />{/if}{#each node.children as child}<svelte:self node={child} {annotations} {selectedId} {onSelect} {onOpenLink} />{/each}</svelte:element>
{/if}

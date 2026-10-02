<script lang="ts">
  import { MousePointer2, Pencil, MoveUpRight, Square, Type, Undo2, Redo2, Trash2, Minus, Plus, Maximize2, Expand } from '@lucide/svelte'
  import type { VisualTool } from './visualModel'
  import { locale } from '../../preferences'
  import { visualText } from './visualI18n'
  export let tool: VisualTool
  export let color: string
  export let width: number
  export let zoom: number
  export let disabled = false
  export let canUndo = false
  export let canRedo = false
  export let selected = false
  export let textSize = false
  export let onTool: (tool: VisualTool) => void
  export let onColor: (color: string) => void
  export let onWidth: (width: number) => void
  export let onUndo: () => void
  export let onRedo: () => void
  export let onDelete: () => void
  export let onZoom: (zoom: number) => void
  export let onFit: () => void
  export let onOpenReview: (() => void) | undefined = undefined
  const tools = [{ id: 'select', label: 'Select', icon: MousePointer2 }, { id: 'freehand', label: 'Pen', icon: Pencil },
    { id: 'arrow', label: 'Arrow', icon: MoveUpRight }, { id: 'rectangle', label: 'Rectangle', icon: Square }, { id: 'text', label: 'Text', icon: Type }] as const
  const tr = (source: string) => visualText($locale, source)
</script>
<div class="flex flex-wrap items-center gap-1.5 rounded-xl border bg-muted/30 p-2" data-visual-toolbar>
  {#if !disabled}
    <div class="flex gap-1" role="group" aria-label={tr('Visual feedback')}>
      {#each tools as item}<button type="button" aria-label={tr(item.label)} title={tr(item.label)} aria-pressed={tool === item.id}
        class="tool" onclick={() => onTool(item.id)}><item.icon class="size-4" /><span class="hidden sm:inline">{tr(item.label)}</span></button>{/each}
    </div>
    <span class="mx-1 h-5 w-px bg-border" aria-hidden="true"></span>
    <label class="flex items-center gap-1.5 px-1 text-xs"><span class="sr-only">{tr('Color')}</span><input type="color" value={color} aria-label={tr('Color')} oninput={(event) => onColor(event.currentTarget.value)} class="size-7 cursor-pointer rounded border border-border bg-transparent p-0.5" /></label>
    <label class="flex items-center gap-1.5 text-xs"><span class="sr-only">{tr(textSize ? 'Text size' : 'Line width')}</span>
      <select aria-label={tr(textSize ? 'Text size' : 'Line width')} value={width} onchange={(event) => onWidth(Number(event.currentTarget.value))} class="h-8 rounded-md border bg-background px-1.5">
        {#each [1, 2, 4, 8, 12, 16, 24, 32] as size}<option value={size}>{size} px</option>{/each}
      </select></label>
    <button type="button" class="tool icon" disabled={!canUndo} aria-label={tr('Undo')} title={tr('Undo')} onclick={onUndo}><Undo2 class="size-4" /></button>
    <button type="button" class="tool icon" disabled={!canRedo} aria-label={tr('Redo')} title={tr('Redo')} onclick={onRedo}><Redo2 class="size-4" /></button>
    <button type="button" class="tool icon" disabled={!selected} aria-label={tr('Delete annotation')} title={tr('Delete annotation')} onclick={onDelete}><Trash2 class="size-4" /></button>
  {/if}
  <div class="ml-auto flex items-center gap-1">
    <button type="button" class="tool icon" disabled={zoom <= .1} aria-label={tr('Zoom out')} onclick={() => onZoom(Math.max(.1, zoom / 1.25))}><Minus class="size-4" /></button>
    <span class="w-12 text-center text-xs tabular-nums">{Math.round(zoom * 100)}%</span>
    <button type="button" class="tool icon" disabled={zoom >= 4} aria-label={tr('Zoom in')} onclick={() => onZoom(Math.min(4, zoom * 1.25))}><Plus class="size-4" /></button>
    <button type="button" class="tool icon" aria-label={tr('Fit canvas')} title={tr('Fit canvas')} onclick={onFit}><Maximize2 class="size-4" /></button>
    {#if onOpenReview}<button type="button" class="tool icon" aria-label={tr('Full screen canvas')} title={tr('Full screen canvas')} onclick={onOpenReview}><Expand class="size-4" /></button>{/if}
  </div>
</div>
<style>
  .tool { display: inline-flex; height: 32px; align-items: center; justify-content: center; gap: 5px; border-radius: 6px; padding: 0 8px; font-size: 12px; }
  .tool:hover { background: var(--muted); }
  .tool[aria-pressed='true'] { background: var(--primary); color: var(--primary-foreground); }
  .tool:focus-visible { outline: 2px solid var(--ring); outline-offset: 2px; }
  .tool:disabled { opacity: .3; pointer-events: none; }
  .icon { width: 32px; padding: 0; }
</style>

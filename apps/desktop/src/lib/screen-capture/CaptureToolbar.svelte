<script lang="ts">
  import {
    Check,
    Circle,
    Copy,
    Ellipsis,
    Grid3X3,
    GripVertical,
    Hash,
    Highlighter,
    Minus,
    MousePointer2,
    MoveUpRight,
    Pencil,
    RectangleHorizontal,
    Redo2,
    Trash2,
    Type,
    Undo2,
    X,
  } from '@lucide/svelte'

  import type { AnnotationTool } from '../screenCapture'

  export let toolbarWidth = 0
  export let toolbarHeight = 0
  export let toolbarStyle = ''
  export let popoverDown = false
  export let activeTool: AnnotationTool = 'select'
  export let stylePanelOpen = false
  export let overflowPanelOpen = false
  export let currentColor = '#ff4d5d'
  export let currentStrokeWidth = 4
  export let colors: string[] = []
  export let strokeWidths: number[] = []
  export let canUndo = false
  export let canRedo = false
  export let canDelete = false
  export let onBeginDrag: (event: MouseEvent) => void = () => {}
  export let onSetTool: (tool: AnnotationTool) => void = () => {}
  export let onToggleStylePanel: () => void = () => {}
  export let onToggleOverflowPanel: () => void = () => {}
  export let onSetColor: (color: string) => void = () => {}
  export let onSetStrokeWidth: (width: number) => void = () => {}
  export let onUndo: () => void = () => {}
  export let onRedo: () => void = () => {}
  export let onDelete: () => void = () => {}
  export let onFinalize: (copyToClipboard: boolean) => void = () => {}
  export let onCancel: () => void = () => {}
</script>

<div
  bind:clientWidth={toolbarWidth}
  bind:clientHeight={toolbarHeight}
  class="capture-toolbar"
  class:popover-down={popoverDown}
  data-capture-ui
  style={toolbarStyle}
>
  <button
    class="toolbar-drag"
    aria-label="拖动工具栏"
    title="拖动工具栏"
    onmousedown={onBeginDrag}
  ><GripVertical size={17} /></button>
  <span class="divider"></span>
  <div class="tool-group">
    <button class:active={activeTool === 'select'} onclick={() => onSetTool('select')} title="选择/修改 · V"><MousePointer2 size={18} /></button>
    <button class:active={activeTool === 'rectangle'} onclick={() => onSetTool('rectangle')} title="矩形 · R"><RectangleHorizontal size={18} /></button>
    <button class:active={activeTool === 'ellipse'} onclick={() => onSetTool('ellipse')} title="圆形 · E"><Circle size={18} /></button>
    <button class:active={activeTool === 'arrow'} onclick={() => onSetTool('arrow')} title="箭头 · A"><MoveUpRight size={18} /></button>
    <button class:active={activeTool === 'pen'} onclick={() => onSetTool('pen')} title="画笔 · P"><Pencil size={18} /></button>
    <button class:active={activeTool === 'text'} onclick={() => onSetTool('text')} title="文字 · T"><Type size={18} /></button>
    <button class:active={activeTool === 'mosaic'} onclick={() => onSetTool('mosaic')} title="马赛克 · B"><Grid3X3 size={18} /></button>
  </div>
  <div class="popup-control">
    <button
      class="style-trigger"
      class:active={stylePanelOpen}
      aria-expanded={stylePanelOpen}
      onclick={onToggleStylePanel}
      title="颜色与粗细"
    ><i style={`--swatch:${currentColor};transform:scaleY(${currentStrokeWidth / 4})`}></i></button>
    {#if stylePanelOpen}
      <div class="toolbar-popover style-popover" aria-label="颜色与线条粗细">
        <div class="palette" aria-label="标注颜色">
          {#each colors as color}
            <button
              class:active={currentColor === color}
              class="color-button"
              style={`--swatch:${color}`}
              onclick={() => onSetColor(color)}
              title={`颜色 ${color}`}
            ></button>
          {/each}
        </div>
        <span class="popover-divider"></span>
        <div class="stroke-picker" aria-label="线条粗细">
          {#each strokeWidths as width}
            <button class:active={currentStrokeWidth === width} onclick={() => onSetStrokeWidth(width)} title={`${width}px`}>
              <i style={`height:${Math.max(2, width / 2)}px`}></i>
            </button>
          {/each}
        </div>
      </div>
    {/if}
  </div>
  <div class="popup-control">
    <button
      class:active={overflowPanelOpen || activeTool === 'line' || activeTool === 'highlight' || activeTool === 'counter'}
      aria-expanded={overflowPanelOpen}
      onclick={onToggleOverflowPanel}
      title="更多工具"
    ><Ellipsis size={18} /></button>
    {#if overflowPanelOpen}
      <div class="toolbar-popover more-popover" aria-label="更多工具">
        <button class:active={activeTool === 'line'} onclick={() => onSetTool('line')} title="直线 · L"><Minus size={18} /></button>
        <button class:active={activeTool === 'highlight'} onclick={() => onSetTool('highlight')} title="高亮 · H"><Highlighter size={18} /></button>
        <button class:active={activeTool === 'counter'} onclick={() => onSetTool('counter')} title="序号 · N"><Hash size={18} /></button>
        <button disabled={!canRedo} onclick={onRedo} title="重做 · Ctrl/⌘ Shift Z"><Redo2 size={18} /></button>
        <button disabled={!canDelete} onclick={onDelete} title="删除选中标注 · Delete"><Trash2 size={18} /></button>
      </div>
    {/if}
  </div>
  <span class="divider"></span>
  <div class="tool-group">
    <button disabled={!canUndo} onclick={onUndo} title="撤销 · Ctrl/⌘ Z"><Undo2 size={18} /></button>
  </div>
  <span class="divider"></span>
  <div class="tool-group actions">
    <button onclick={() => onFinalize(true)} title="复制并插入"><Copy size={18} /></button>
    <button class="confirm" onclick={() => onFinalize(false)} title="插入文档 · Enter"><Check size={19} /></button>
    <button class="cancel" onclick={onCancel} title="取消 · Esc"><X size={19} /></button>
  </div>
</div>

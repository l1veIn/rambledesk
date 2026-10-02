<script lang="ts">
  import type { VisualFeedbackAnnotation } from '../../generated/feedback'
  import type { AttachmentView } from '../../feedback'
  import { arrowHead, pointsPath } from './visualModel'
  import { visualAttachmentText } from './visualAttachmentText'
  export let annotations: VisualFeedbackAnnotation[]
  export let selectedId: string | null = null
  export let attachments: readonly Pick<AttachmentView, 'attachment_id'>[] = []
</script>
{#each annotations as mark (mark.id)}
  {@const a = mark.points[0]}
  {@const b = mark.points[1]}
  <g data-visual-mark-id={mark.id} stroke={mark.color} stroke-width={mark.stroke_width} stroke-linecap="round" stroke-linejoin="round" fill="none">
    {#if mark.kind === 'freehand'}<polyline points={pointsPath(mark.points)} />
    {:else if mark.kind === 'arrow'}<line x1={a.x} y1={a.y} x2={b.x} y2={b.y} /><polyline points={pointsPath(arrowHead(a, b, mark.stroke_width))} />
    {:else if mark.kind === 'rectangle'}<rect x={Math.min(a.x, b.x)} y={Math.min(a.y, b.y)} width={Math.abs(b.x - a.x)} height={Math.abs(b.y - a.y)} />
    {:else}<text x={a.x} y={a.y} fill={mark.color} stroke="none" font-family="Arial, sans-serif" font-size={mark.stroke_width} dominant-baseline="text-before-edge">
      {#each visualAttachmentText(mark.text, attachments).split('\n') as line, index}<tspan x={a.x} y={a.y + index * mark.stroke_width * 1.25}>{line}</tspan>{/each}
    </text>{/if}
    {#if selectedId === mark.id}<circle cx={a.x} cy={a.y} r={Math.max(5, mark.stroke_width)} stroke="white" stroke-width="2" fill={mark.color} />{/if}
  </g>
{/each}

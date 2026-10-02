<script lang="ts">
  import { Trash2 } from '@lucide/svelte'
  import type { VisualFeedbackAnnotation, VisualFeedbackData } from '../../generated/feedback'
  import WorkbenchTextField from '../../input/WorkbenchTextField.svelte'
  import { unavailableVoiceInputState, useVoiceInput, workbenchFieldVoiceTarget } from '../../speech/voiceInputContext'
  import { locale } from '../../preferences'
  import { visualText } from './visualI18n'
  export let data: VisualFeedbackData
  export let annotation: VisualFeedbackAnnotation
  export let disabled = false
  export let onUpdate: (annotation: VisualFeedbackAnnotation) => void
  export let onDelete: () => void
  const voiceState = useVoiceInput()?.state ?? unavailableVoiceInputState
  const tr = (source: string) => visualText($locale, source)
  $: bodyTarget = workbenchFieldVoiceTarget($voiceState, { workbenchType: 'visual_feedback', version: 1,
    field: 'body', entityId: annotation.id, sourceVersion: data.source_version, label: tr('Your comment') })
  $: textTarget = workbenchFieldVoiceTarget($voiceState, { workbenchType: 'visual_feedback', version: 1,
    field: 'text', entityId: annotation.id, sourceVersion: data.source_version, label: tr('Text on canvas') })
</script>
<div class="grid gap-3 rounded-xl border bg-background p-3" data-visual-annotation-editor>
  <div class="flex items-center gap-2"><span class="flex-1 text-xs font-medium">{tr('Select')} · {annotation.points[0].x}, {annotation.points[0].y}</span>
    {#if !disabled}<button type="button" class="rounded p-1.5 text-muted-foreground hover:bg-destructive/10 hover:text-destructive" aria-label={tr('Delete annotation')} onclick={onDelete}><Trash2 class="size-4" /></button>{/if}
  </div>
  {#if annotation.kind === 'text'}<WorkbenchTextField value={annotation.text} label={tr('Text on canvas')} voiceLabel={tr('Speak annotation text')}
    target={textTarget} maxLength={2000} {disabled} data-visual-field="text" onChange={(text) => { if (!disabled) onUpdate({ ...annotation, text }) }} />{/if}
  <WorkbenchTextField value={annotation.body} label={tr('Your comment')} voiceLabel={tr('Speak comment')}
    target={bodyTarget} maxLength={4000} {disabled} data-visual-field="body" onChange={(body) => { if (!disabled) onUpdate({ ...annotation, body }) }} />
</div>

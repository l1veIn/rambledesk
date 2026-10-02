import type { SpeechTarget } from './speechDraftQueue'
import { sameSpeechTarget } from './speechTargets'

type Translate = (source: string) => string

export function speechTargetLabel(target: SpeechTarget, tr: Translate): string {
  const destination = target.destination
  const detail = destination.kind === 'document'
    ? destination.action?.title ?? tr('Feedback document')
    : destination.kind === 'workbench_field'
      ? destination.label
      : destination.kind === 'review_annotation'
      ? `${destination.paragraphLabel} · ${tr(destination.field === 'body' ? 'Comment' : 'Suggested wording')}`
      : destination.kind === 'question_answer'
        ? `${destination.questionLabel} · ${tr('Your answer')}`
        : destination.kind === 'web_review_annotation'
          ? `${destination.elementLabel} · ${tr('Comment')}`
          : tr('Unavailable input target')
  return `${target.requestTitle} · ${detail}`
}

export function sameVoiceInputTarget(first: SpeechTarget | null | undefined, second: SpeechTarget | null | undefined): boolean {
  return !!first && !!second && first.destination.kind !== 'unknown' && second.destination.kind !== 'unknown' && sameSpeechTarget(first, second)
}

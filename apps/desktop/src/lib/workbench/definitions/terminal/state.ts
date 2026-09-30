import type { ParagraphMark, QuestionAnswer, ReviewAnnotation, TerminalTrialSession, WebReviewAnnotation, WorkbenchState } from '../../../generated/feedback'
import { record, text, nullableText, nullableInteger } from '../stateShape'

type TerminalState = Extract<WorkbenchState, { type: 'terminal' }>

function terminalSession(value: unknown): value is TerminalTrialSession {
  return record(value) && text(value.id) && text(value.cwd) && text(value.shell)
    && Number.isInteger(value.cols) && Number.isInteger(value.rows)
    && ['running', 'exited', 'stopped'].includes(value.status as string) && nullableInteger(value.exit_code)
    && text(value.output) && text(value.screen) && typeof value.truncated === 'boolean'
}

export function readTerminalState(value: unknown): TerminalState | null {
  return record(value) && value.type === 'terminal' && Array.isArray(value.sessions) && value.sessions.every(terminalSession)
    ? value as TerminalState : null
}

import { getContext, setContext } from 'svelte'
import { readable, type Readable } from 'svelte/store'
import type { AttachmentView } from '../feedback'
import type { InputTarget } from '../domain/inputTarget'

export type InputToolsState = {
  requestId: string
  disabled: boolean
  busy: boolean
  canCapture: boolean
  canPaste: boolean
  attachments: readonly AttachmentView[]
}

/** Acquisition belongs to the request; every invocation carries its input destination. */
export type InputToolsContext = {
  state: Readable<InputToolsState>
  capture(target: InputTarget): void | Promise<void>
  paste(target: InputTarget): void | Promise<void>
  files(target: InputTarget, files: readonly File[]): void | Promise<void>
  preview(attachmentId: string): void
  reportError(cause: unknown): void
}

export const INPUT_TOOLS_CONTEXT = Symbol('rambledesk.input-tools')
export const unavailableInputToolsState = readable<InputToolsState>({
  requestId: '', disabled: true, busy: false, canCapture: false, canPaste: false, attachments: [],
})
export function provideInputTools(context: InputToolsContext) { setContext(INPUT_TOOLS_CONTEXT, context) }
export function useInputTools(): InputToolsContext | undefined { return getContext(INPUT_TOOLS_CONTEXT) }

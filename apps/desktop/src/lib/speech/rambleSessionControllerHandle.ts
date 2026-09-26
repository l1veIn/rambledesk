/** The input owner has finished writing, needs human review, or failed to stop. */
export type FeedbackPreparation =
  | { kind: 'ready' }
  | { kind: 'pending-speech' }
  | { kind: 'failed'; message: string }

export type RambleSessionControllerHandle = {
  startInput(): Promise<void>
  pauseInput(): Promise<void>
  inputBusy(): boolean
  toggleRamble(): Promise<void>
  exitRamble(): Promise<void>
  importClipboardNow(target?: import('../domain/inputTarget').InputTarget): Promise<void>
  prepareFeedback(requestId: string): Promise<FeedbackPreparation>
}

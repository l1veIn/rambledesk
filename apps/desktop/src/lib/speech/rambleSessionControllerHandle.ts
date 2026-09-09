/** The input owner has finished writing, needs human review, or failed to stop. */
export type FeedbackPreparation =
  | { kind: 'ready' }
  | { kind: 'pending-speech' }
  | { kind: 'failed'; message: string }

export type RambleSessionControllerHandle = {
  toggleRamble(): Promise<void>
  exitRamble(): Promise<void>
  importClipboardNow(): Promise<void>
  prepareFeedback(requestId: string): Promise<FeedbackPreparation>
}

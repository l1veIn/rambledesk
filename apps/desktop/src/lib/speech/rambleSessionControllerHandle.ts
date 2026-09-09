/** Contract the Ramble session controller exposes to the composition root. */
export type RambleSessionControllerHandle = {
  toggleRamble(): Promise<void>
  exitRamble(): Promise<void>
  importClipboardNow(): Promise<void>
  resetVoiceUi(): void
  resetRambleUi(): void
  hasPendingSpeech(requestId: string): boolean
  settleSpeechDrafts(): Promise<void>
}

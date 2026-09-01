import type { JSONContent } from '@tiptap/core'

import type { DraftOperation } from '../draftOperations'
import type { SpeechCleanupSegment } from '../speechBlockMetadata'

export type SavePhase = 'idle' | 'unsaved' | 'saving' | 'saved' | 'error'
export type RamblePhase = 'idle' | 'starting' | 'active' | 'paused' | 'stopping' | 'error'
export type VoicePhase = 'idle' | 'starting' | 'listening' | 'processing' | 'stopping' | 'error'
export type SubmitStage = 'idle' | 'cooking' | 'publishing'
export type SettingsSection =
  | 'general'
  | 'permissions'
  | 'notifications'
  | 'voice'
  | 'post-processing'
  | 'shortcuts'
  | 'adapters'
  | 'about'

export type ResumePrompt = {
  request_id: string
  host_id: string
  host_label: string
  title: string
  body: string
  resume_prompt: string
  reason: 'completed' | 'cancelled'
}

export type { ApplicationHostProfileView as HostProfile } from '../generated/feedback'

export type FeedbackEditorHandle = {
  removeAttachmentReference(attachmentId: string): void
  applyDraftOperation(operation: DraftOperation): boolean
  pendingSpeechSegments(): SpeechCleanupSegment[]
  replaceSpeechSegments(
    replacements: Array<{ segmentId: string; originalText: string; nextText: string }>,
  ): boolean
}

export type RambleSessionControllerHandle = {
  toggleRamble(): Promise<void>
  exitRamble(): Promise<void>
  importClipboardNow(): Promise<void>
  resetVoiceUi(): void
  resetRambleUi(): void
}

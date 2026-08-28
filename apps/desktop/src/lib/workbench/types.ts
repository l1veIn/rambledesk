import type { JSONContent } from '@tiptap/core'

import type { DraftOperation } from '../draftOperations'
import type { AttachmentView } from '../feedback'
import type { SpeechCleanupSegment } from '../speechBlockMetadata'

export type SavePhase = 'idle' | 'unsaved' | 'saving' | 'saved' | 'error'
export type RamblePhase = 'idle' | 'starting' | 'active' | 'paused' | 'stopping' | 'error'
export type VoicePhase = 'idle' | 'starting' | 'listening' | 'processing' | 'stopping' | 'error'
export type SubmitStage = 'idle' | 'cooking' | 'publishing'
export type SettingsSection = 'general' | 'notifications' | 'voice' | 'adapters' | 'about'

export type ResumePrompt = {
  request_id: string
  host_id: string
  host_label: string
  title: string
  body: string
  resume_prompt: string
  reason: 'completed' | 'cancelled'
}

export type HostProfile = {
  id: string
  label: string
  icon_svg: string
  default_adapter: 'generic_mcp' | 'pi_native'
  continuation_mode: 'not_required' | 'manual' | 'native'
}

export type FeedbackEditorHandle = {
  insertAttachments(attachments: AttachmentView[]): boolean
  appendTranscript(text: string): void
  appendClipboardCapture(text: string, label: string): boolean
  appendCapturedAttachment(attachment: AttachmentView, label: string): boolean
  removeAttachmentReference(attachmentId: string): void
  applyExternalMarkdown(markdown: string): boolean
  applyExternalDocument(document: JSONContent): boolean
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

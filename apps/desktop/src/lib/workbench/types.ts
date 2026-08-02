import type { AttachmentView } from '../feedback'

export type SavePhase = 'idle' | 'unsaved' | 'saving' | 'saved' | 'error'
export type RamblePhase = 'idle' | 'starting' | 'active' | 'paused' | 'stopping' | 'error'
export type VoicePhase = 'idle' | 'starting' | 'listening' | 'processing' | 'stopping' | 'error'
export type SettingsSection = 'general' | 'adapters'

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
}

export type RambleSessionControllerHandle = {
  toggleRamble(): Promise<void>
  exitRamble(): Promise<void>
  importClipboardNow(): Promise<void>
  resetVoiceUi(): void
  resetRambleUi(): void
}

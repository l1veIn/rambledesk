export type ResumePrompt = {
  request_id: string
  host_id: string
  host_label: string
  title: string
  body: string
  resume_prompt: string
  reason: 'completed' | 'cancelled'
  /** Only explicit product defaults are localized. Unmarked host content is verbatim. */
  default_presentation?: boolean
}

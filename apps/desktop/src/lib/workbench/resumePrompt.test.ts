import { describe, expect, it } from 'vitest'

import type { FeedbackWorkspaceView } from '../feedback'
import type { HostProfile } from './types'
import { buildResumePrompt, shouldShowResumePromptButton } from './resumePrompt'

const workspace = {
  request: {
    request_id: '019fc1d9-51e7-7eb2-b196-e9266947fc41',
    host_id: 'codex',
  },
} as FeedbackWorkspaceView

const hostProfile: HostProfile = {
  id: 'codex',
  label: 'Codex',
  icon_svg: '',
  default_adapter: 'generic_mcp',
  continuation_mode: 'manual',
}

describe('resumePrompt helpers', () => {
  it('rebuilds the continuation prompt for a completed request', () => {
    const prompt = buildResumePrompt(workspace, hostProfile, (source, values) =>
      values?.host ? source.replace('{host}', String(values.host)) : source,
    )

    expect(prompt.request_id).toBe(workspace.request.request_id)
    expect(prompt.host_label).toBe('Codex')
    expect(prompt.reason).toBe('completed')
    expect(prompt.resume_prompt).toContain(workspace.request.request_id)
    expect(prompt.resume_prompt).toContain('get_feedback')
  })

  it('shows the manual reopen button only for submitted feedback packages', () => {
    const packageResult = {
      available: true,
    }

    expect(shouldShowResumePromptButton(packageResult, 'feedback_submitted')).toBe(true)
    expect(shouldShowResumePromptButton(packageResult, 'cancelled')).toBe(false)
    expect(shouldShowResumePromptButton(packageResult, 'approved')).toBe(false)
    expect(shouldShowResumePromptButton(null, 'feedback_submitted')).toBe(false)
  })
})

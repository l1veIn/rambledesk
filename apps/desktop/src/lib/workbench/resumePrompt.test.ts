import { describe, expect, it } from 'vitest'

import type { FeedbackWorkspaceView } from '../feedback'
import type { HostProfile } from '../domain/hostProfile'
import { buildResumePrompt, resumePromptPresentation, shouldShowResumePromptButton } from './resumePrompt'
import { t } from '../i18n'

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
    expect(prompt.default_presentation).toBe(true)
    expect(prompt.resume_prompt).toContain(workspace.request.request_id)
    expect(prompt.resume_prompt).toContain('get_feedback')
  })

  it('localizes a marked cancellation while keeping the host label as data', () => {
    const prompt = { ...buildResumePrompt(workspace, hostProfile, (source) => source),
      reason: 'cancelled' as const, host_label: '我的宿主',
    }
    expect(resumePromptPresentation(prompt, (source, values) => t('en', source, values))).toEqual({
      title: 'Feedback cancelled · return to host',
      body: 'Return to 我的宿主 and click the waiting confirmation to finish. Only paste the fallback prompt below and call get_feedback if the host is not waiting.',
    })
    expect(resumePromptPresentation(prompt, (source, values) => t('zh-CN', source, values)).title)
      .toBe('反馈已取消 · 回到宿主点继续')
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

  it('uses the trusted request binding even when the owning Agent is not in navigation', () => {
    expect(shouldShowResumePromptButton({ available: true }, 'feedback_submitted', 'managed-session')).toBe(false)
    expect(shouldShowResumePromptButton({ available: true }, 'feedback_submitted', undefined)).toBe(true)
  })
})

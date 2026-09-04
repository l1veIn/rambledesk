import { describe, expect, it } from 'vitest'

import type { FeedbackWorkspaceView } from '../feedback'
import type { HostProfile } from './types'
import type { HostSessionSummary, SessionManagement } from '../generated/feedback'
import { buildResumePrompt, requestSessionManagement, shouldShowResumePromptButton } from './resumePrompt'

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

  it('keeps manual continuation for external sessions while suppressing managed sessions of the same backend', () => {
    const managed: SessionManagement = { kind: 'managed', protocol: 'acp', agent_config_id: 'config', cwd: '/repo', remote_session_id: 'remote' }
    const session = (id: string, management: SessionManagement): HostSessionSummary => ({
      session_id: id, host_id: 'codex', host_session_id: id, title: id, management,
      source_hint: null, request_count: 1, pending_count: 0, updated_at: 'today', pinned_at: null, archived_at: null, host_pinned_at: null,
    })
    const sessions = [session('managed', managed), session('external', { kind: 'external' })]
    const managedRequest = { host_id: 'codex', host_session_id: 'managed' }
    const externalRequest = { host_id: 'codex', host_session_id: 'external' }
    expect(shouldShowResumePromptButton({ available: true }, 'feedback_submitted', requestSessionManagement(managedRequest, sessions))).toBe(false)
    expect(shouldShowResumePromptButton({ available: true }, 'feedback_submitted', requestSessionManagement(externalRequest, sessions))).toBe(true)
    expect(requestSessionManagement({ host_id: 'another', host_session_id: 'managed' }, sessions)).toBeUndefined()
    expect(requestSessionManagement(null, sessions)).toBeUndefined()
  })
})

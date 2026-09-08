import { get } from 'svelte/store'
import { describe, expect, it } from 'vitest'

import type { FeedbackRequestView } from '../feedback'
import { previewFixtures } from '../previewFixtures'
import { createWorkspaceSession } from './workspaceSession'

function requestView(overrides: Partial<FeedbackRequestView> = {}): FeedbackRequestView {
  return {
    ...previewFixtures.workspace.request,
    execution_mode: 'poll',
    feedback: null,
    status: 'completed',
    updated_at: '2026-09-09T10:00:00Z',
    resolution: 'approved',
    ...overrides,
  }
}

describe('workspace session', () => {
  it('opens a request and exposes its projections', () => {
    const session = createWorkspaceSession()
    session.open(previewFixtures.workspace, { markdown: 'package' })

    expect(session.requestId()).toBe(previewFixtures.workspace.request.request_id)
    expect(session.request()).toEqual(previewFixtures.workspace.request)
    expect(get(session).workspace).toEqual(previewFixtures.workspace)
    expect(get(session).completedResult).toBeNull()
    expect(get(session).publishedFeedback).toEqual({ markdown: 'package' })
    expect(session.isTerminal()).toBe(false)
  })

  it('folds a terminal result into the matching request only', () => {
    const session = createWorkspaceSession()
    session.open(previewFixtures.workspace)

    expect(session.applyMutationResult(requestView({ request_id: 'other-request' }))).toBe(false)
    expect(get(session).completedResult).toBeNull()

    const result = requestView()
    expect(session.applyMutationResult(result)).toBe(true)
    expect(session.request()?.status).toBe('completed')
    expect(session.request()?.resolution).toBe('approved')
    expect(session.isTerminal()).toBe(true)
    expect(session.feedbackResult()).toBe(result.feedback ?? null)
  })

  it('prefers a completed result over the workspace projection', () => {
    const session = createWorkspaceSession()
    session.open(previewFixtures.workspace)
    const completed = requestView({ feedback: null })

    session.setCompleted(completed)

    expect(session.feedbackResult()).toBe(completed.feedback)
  })

  it('locks interaction while a submission mutation runs', () => {
    const session = createWorkspaceSession()
    session.open(previewFixtures.workspace)
    expect(session.interactionLocked()).toBe(false)

    session.setSubmitting(true)
    expect(session.interactionLocked()).toBe(true)
    session.setSubmitting(false)
    session.beginCancel()
    expect(session.interactionLocked()).toBe(true)
    session.endCancel()
    session.beginApprove()
    expect(session.interactionLocked()).toBe(true)
    session.endApprove()
    expect(session.interactionLocked()).toBe(false)
  })

  it('updates the open draft and ignores it when no request is open', () => {
    const session = createWorkspaceSession()
    const draft = { ...previewFixtures.workspace.draft, body_markdown: 'edited' }

    session.setDraft(draft)
    expect(get(session).workspace).toBeNull()

    session.open(previewFixtures.workspace)
    session.setDraft(draft)
    expect(get(session).workspace?.draft).toEqual(draft)
  })

  it('closes every field', () => {
    const session = createWorkspaceSession()
    session.open(previewFixtures.workspace, { markdown: 'package' })
    session.setLoading(true)
    session.setSubmitting(true)
    session.setSubmitStage('publishing')

    session.close()

    expect(get(session)).toMatchObject({
      workspace: null,
      completedResult: null,
      publishedFeedback: null,
      loadingWorkspace: false,
      submitting: false,
      submitStage: 'idle',
      approving: false,
      cancelling: false,
    })
  })
})

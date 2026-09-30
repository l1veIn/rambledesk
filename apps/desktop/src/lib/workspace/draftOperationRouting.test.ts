import { describe, expect, it } from 'vitest'

import {
  requestTaskViewDescriptor,
  sessionViewDescriptor,
  workbenchReviewViewDescriptor,
} from './viewDescriptors'
import {
  shouldAdoptTaskBackgroundDraft,
  shouldUseForegroundDraftEditor,
} from './draftOperationRouting'

describe('workspace draft operation routing', () => {
  it('routes review writes to its live editor and prevents stale request bindings', () => {
    const base = {
      activeView: workbenchReviewViewDescriptor('request-a'), workbenchMounted: true,
      editorReady: true, workspaceRequestId: 'request-a', requestId: 'request-a',
    }
    expect(shouldUseForegroundDraftEditor(base)).toBe(true)
    expect(shouldUseForegroundDraftEditor({ ...base, requestId: 'request-b' })).toBe(false)
    expect(shouldUseForegroundDraftEditor({ ...base, workspaceRequestId: 'request-b' })).toBe(false)
    expect(shouldUseForegroundDraftEditor({ ...base, editorReady: false })).toBe(false)
    expect(shouldUseForegroundDraftEditor({ ...base, workbenchMounted: false })).toBe(false)
    expect(shouldUseForegroundDraftEditor({ ...base, activeView: workbenchReviewViewDescriptor('request-b') })).toBe(false)
  })

  it('adopts a delayed background write only for the review still showing that request', () => {
    const review = workbenchReviewViewDescriptor('request-a')
    expect(shouldAdoptTaskBackgroundDraft(review, 'request-a', 'request-a', true)).toBe(true)
    expect(shouldAdoptTaskBackgroundDraft(review, 'request-a', 'request-a')).toBe(false)
    expect(shouldAdoptTaskBackgroundDraft(review, 'request-a', 'request-b')).toBe(false)
    expect(shouldAdoptTaskBackgroundDraft(review, 'request-b', 'request-a')).toBe(false)
  })
  it('uses the foreground editor only for a mounted Session view with a real handle', () => {
    const base = {
      workbenchMounted: true,
      editorReady: true,
      workspaceRequestId: 'request-a',
      requestId: 'request-a',
    }

    expect(
      shouldUseForegroundDraftEditor({
        ...base,
        activeView: sessionViewDescriptor('codex', 'session-a'),
      }),
    ).toBe(true)
    expect(
      shouldUseForegroundDraftEditor({
        ...base,
        activeView: requestTaskViewDescriptor('request-a'),
      }),
    ).toBe(false)
    expect(
      shouldUseForegroundDraftEditor({
        ...base,
        activeView: sessionViewDescriptor('codex', 'session-a'),
        editorReady: false,
      }),
    ).toBe(false)
  })

  it('adopts a background draft only while the same Task remains active', () => {
    const task = requestTaskViewDescriptor('request-a')

    expect(shouldAdoptTaskBackgroundDraft(task, 'request-a', 'request-a')).toBe(true)
    expect(shouldAdoptTaskBackgroundDraft(task, 'request-b', 'request-a')).toBe(false)
    expect(
      shouldAdoptTaskBackgroundDraft(
        sessionViewDescriptor('codex', 'session-a'),
        'request-a',
        'request-a',
      ),
    ).toBe(false)
  })
})

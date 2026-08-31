import { describe, expect, it } from 'vitest'

import {
  requestTaskViewDescriptor,
  sessionViewDescriptor,
} from './viewDescriptors'
import {
  shouldAdoptTaskBackgroundDraft,
  shouldUseForegroundDraftEditor,
} from './draftOperationRouting'

describe('workspace draft operation routing', () => {
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

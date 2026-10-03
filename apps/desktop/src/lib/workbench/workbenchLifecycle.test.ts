import { describe, expect, it, vi } from 'vitest'
import type { FeedbackWorkspaceView } from '../feedback'
import type { WorkbenchState } from '../generated/feedback'
import { TestApplicationTransport } from '../application/testApplicationTransport'
import { createWorkbenchLifecycle } from './workbenchLifecycle'
import { previewFixtures } from '../preview/previewFixtures'

describe('request-owned workbench preparation intent', () => {
  it('forwards cancellation to visual feedback without requiring valid publication text or image resources', async () => {
    const workspace: FeedbackWorkspaceView = { ...structuredClone(previewFixtures.workspace),
      workbench: { type: 'visual_feedback', version: 1, data: { title: 'Sketch', source_version: 'v1', width: 640, height: 400, image_file_name: 'missing.png' } } }
    const state: WorkbenchState = { type: 'visual_feedback', composite_attachment_id: null,
      annotations: [{ id: 'text_one', kind: 'text', points: [{ x: 20, y: 20 }], color: '#e5484d', stroke_width: 24, text: '', body: '' }] }
    const transport = new TestApplicationTransport(), updateState = vi.fn(), persistGeneratedAttachment = vi.fn(async () => { throw new Error('Upload unavailable') })
    const lifecycle = createWorkbenchLifecycle({ transport, getWorkspace: () => workspace, getState: () => state,
      updateState, isEditable: () => true, onBusy: () => {}, persistGeneratedAttachment })
    await lifecycle.prepareSubmission(workspace.request.request_id, 'cancel')
    expect(transport.calls).toEqual([]); expect(updateState).not.toHaveBeenCalled(); expect(persistGeneratedAttachment).not.toHaveBeenCalled()
    await expect(lifecycle.prepareSubmission(workspace.request.request_id)).rejects.toThrow('annotations are invalid')
    lifecycle.dispose()
  })
  it('rejects stale requests before controller cancellation can prepare or mutate them', async () => {
    const workspace = structuredClone(previewFixtures.workspace), transport = new TestApplicationTransport()
    const lifecycle = createWorkbenchLifecycle({ transport, getWorkspace: () => workspace, getState: () => null,
      updateState: vi.fn(), isEditable: () => true, onBusy: vi.fn() })
    await expect(lifecycle.prepareSubmission('other-request', 'cancel')).rejects.toThrow('request changed')
    expect(transport.calls).toEqual([]); lifecycle.dispose()
  })
})

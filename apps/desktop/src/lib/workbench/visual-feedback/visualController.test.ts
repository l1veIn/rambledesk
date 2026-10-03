import { describe, expect, it, vi } from 'vitest'
import type { FeedbackWorkspaceView } from '../../feedback'
import type { ApplicationTransport } from '../../application/applicationTransport'
import type { VisualFeedbackData, WorkbenchState } from '../../generated/feedback'
import type { WorkbenchControllerContext } from '../definitions/contracts'
import { createVisualWorkbenchController } from './visualController'
import { emptyVisualState, type VisualState } from './visualModel'

const data: VisualFeedbackData = { title: 'Sketch', source_version: 'snapshot-1', width: 960, height: 600, image_file_name: null }
const mark = { id: 'mark_one', kind: 'arrow' as const, points: [{ x: 20, y: 30 }, { x: 200, y: 180 }], color: '#e5484d', stroke_width: 4, text: '', body: '' }
function fixture(input = data) {
  let state: VisualState | null = { ...emptyVisualState(), annotations: [mark] }, editable = true
  let workspace = { request: { request_id: 'request' }, workbench: { type: 'visual_feedback', version: 1, data: input },
    request_attachments: [], attachments: [] } as unknown as FeedbackWorkspaceView
  const call = vi.fn(async () => new ArrayBuffer(4)), exportPng = vi.fn(async () => new ArrayBuffer(8))
  const decodeImage = vi.fn(async () => ({ naturalWidth: input.width, naturalHeight: input.height }) as HTMLImageElement)
  const persist = vi.fn(async () => {
    const attachment = { attachment_id: `generated_${persist.mock.calls.length}`, file_name: 'visual-feedback.png', media_type: 'image/png', byte_size: 8, sha256: 'hash', position: workspace.attachments.length }
    workspace = { ...workspace, attachments: [...workspace.attachments, attachment] }; return attachment
  })
  const setBusy = vi.fn(), updateState = vi.fn((next: WorkbenchState) => { if (next.type === 'visual_feedback') state = next })
  const context: WorkbenchControllerContext = { requestId: 'request', getState: () => state, getWorkspace: () => workspace,
    updateState, isEditable: () => editable, setBusy, runtime: { transport: { call } as unknown as ApplicationTransport, persistGeneratedAttachment: persist } }
  const controller = createVisualWorkbenchController(context, { exportPng, decodeImage })
  return { controller, context, call, exportPng, decodeImage, persist, setBusy, updateState, state: () => state,
    edit: (next: VisualState | null) => state = next, lock: () => editable = false, workspace: () => workspace, replace: (next: FeedbackWorkspaceView) => workspace = next }
}
describe('request-owned visual image preparation', () => {
  it('composes text against the current workspace attachment membership', async () => {
    const f = fixture(), attachment = { attachment_id: 'supporting_file', file_name: 'spec.pdf', media_type: 'application/pdf', byte_size: 10, sha256: 'hash', position: 0 }
    f.replace({ ...f.workspace(), attachments: [attachment] })
    await f.controller.prepareSubmission()
    expect(f.exportPng).toHaveBeenCalledWith(data, [mark], null, [attachment])
  })
  it('prepares without a mounted view, caches an unchanged retry, and regenerates a removed image', async () => {
    const f = fixture(); await f.controller.prepareSubmission()
    expect(f.exportPng).toHaveBeenCalledWith(data, [mark], null, [])
    expect(f.state()?.composite_attachment_id).toBe('generated_1')
    await f.controller.prepareSubmission(); expect(f.persist).toHaveBeenCalledTimes(1)
    f.replace({ ...f.workspace(), attachments: [] }); await f.controller.prepareSubmission()
    expect(f.persist).toHaveBeenCalledTimes(2); expect(f.state()?.composite_attachment_id).toBe('generated_2')
  })
  it('allows notes-only preparation from an empty draft', async () => {
    const f = fixture(); f.edit(null); await f.controller.prepareSubmission()
    expect(f.state()).toEqual({ ...emptyVisualState(), composite_attachment_id: 'generated_1' })
  })
  it('releases busy and retries after an export or persistence error', async () => {
    const f = fixture(); f.exportPng.mockRejectedValueOnce(new Error('Canvas failed'))
    await expect(f.controller.prepareSubmission()).rejects.toThrow('Canvas failed')
    expect(f.persist).not.toHaveBeenCalled(); expect(f.setBusy).toHaveBeenLastCalledWith(false)
    f.persist.mockRejectedValueOnce(new Error('Upload failed'))
    await expect(f.controller.prepareSubmission()).rejects.toThrow('Upload failed')
    expect(f.state()?.composite_attachment_id).toBeNull(); await f.controller.prepareSubmission()
    expect(f.state()?.composite_attachment_id).toBe('generated_2')
  })
  it('never publishes stale annotation output after edits, disposal or request switching', async () => {
    for (const stale of ['edit', 'dispose', 'switch'] as const) {
      const f = fixture(); let finish!: (png: ArrayBuffer) => void
      f.exportPng.mockImplementationOnce(() => new Promise((resolve) => finish = resolve))
      const preparing = f.controller.prepareSubmission(); await vi.waitFor(() => expect(f.exportPng).toHaveBeenCalled())
      if (stale === 'edit') f.edit({ ...emptyVisualState(), annotations: [{ ...mark, body: 'New opinion' }] })
      if (stale === 'dispose') f.controller.dispose()
      if (stale === 'switch') f.replace({ ...f.workspace(), request: { ...f.workspace().request, request_id: 'other' } })
      finish(new ArrayBuffer(8)); await expect(preparing).rejects.toThrow('changed')
      expect(f.persist).not.toHaveBeenCalled(); expect(f.updateState).not.toHaveBeenCalled()
    }
  })
  it('loads the frozen image once in readonly mode, validates dimensions, and rejects ambiguous sources', async () => {
    const input = { ...data, image_file_name: 'original.png' }, f = fixture(input)
    const attachment = { attachment_id: 'original', file_name: 'original.png', media_type: 'image/png', byte_size: 4, sha256: 'hash', position: 0 }
    f.replace({ ...f.workspace(), request_attachments: [attachment] }); f.lock()
    await f.controller.loadBackground(input); await f.controller.loadBackground(input)
    expect(f.call).toHaveBeenCalledExactlyOnceWith('readRequestAttachment', { request_id: 'request', attachment_id: 'original' })
    const mismatch = fixture(input); mismatch.replace({ ...mismatch.workspace(), request_attachments: [attachment] }); mismatch.decodeImage.mockResolvedValueOnce({ naturalWidth: 1, naturalHeight: 1 } as HTMLImageElement)
    await expect(mismatch.controller.loadBackground(input)).rejects.toThrow('dimensions')
    const ambiguous = fixture(input); ambiguous.replace({ ...ambiguous.workspace(), request_attachments: [attachment, { ...attachment, attachment_id: 'second' }] })
    await expect(ambiguous.controller.loadBackground(input)).rejects.toThrow('unavailable'); expect(ambiguous.call).not.toHaveBeenCalled()
  })
  it('does not bypass the session persistence queue when a client lacks the generated attachment capability', async () => {
    const f = fixture(); f.context.runtime.persistGeneratedAttachment = undefined
    await expect(f.controller.prepareSubmission()).rejects.toThrow('unavailable')
    expect(f.call).not.toHaveBeenCalled(); expect(f.exportPng).not.toHaveBeenCalled(); expect(f.setBusy).toHaveBeenLastCalledWith(false)
  })
  it.each([['\u0085', false], ['\ufeff', true]] as const)('prepares text %j using the backend Unicode whitespace boundary', async (value, visible) => {
    const f = fixture(); f.edit({ ...emptyVisualState(), annotations: [{ ...mark, kind: 'text', points: [{ x: 20, y: 30 }], text: value, body: value }] })
    if (visible) { await f.controller.prepareSubmission(); expect(f.persist).toHaveBeenCalledOnce() }
    else { await expect(f.controller.prepareSubmission()).rejects.toThrow('annotations are invalid'); expect(f.persist).not.toHaveBeenCalled() }
  })
  it('does not write a completed upload into a switched request or changed draft', async () => {
    const f = fixture(); let finish!: (value: Awaited<ReturnType<typeof f.persist>>) => void
    f.persist.mockImplementationOnce(() => new Promise((resolve) => finish = resolve))
    const preparing = f.controller.prepareSubmission(); await vi.waitFor(() => expect(f.persist).toHaveBeenCalled())
    f.replace({ ...f.workspace(), request: { ...f.workspace().request, request_id: 'other' } })
    finish({ attachment_id: 'uploaded', file_name: 'visual-feedback.png', media_type: 'image/png', byte_size: 8, sha256: 'hash', position: 0 })
    await expect(preparing).rejects.toThrow('changed'); expect(f.updateState).not.toHaveBeenCalled()
  })
})

import { describe, expect, it, vi } from 'vitest'
import type { ApplicationTransport } from '../../application/applicationTransport'
import type { FeedbackWorkspaceView } from '../../feedback'
import type { MediaReviewData, WorkbenchState } from '../../generated/feedback'
import type { WorkbenchControllerContext } from '../definitions/contracts'
import { createMediaReviewController } from './mediaController'

const data: MediaReviewData = { title: 'Clip', source_version: 'clip-v1', media_kind: 'video', media_file_name: 'clip.webm', duration_ms: 8000 }
function fixture() {
  let state: WorkbenchState | null = null, editable = true
  let workspace = { request: { request_id: 'request' }, workbench: { type: 'media_review', version: 1, data },
    request_attachments: [{ attachment_id: 'source', file_name: 'clip.webm', media_type: 'video/webm' }], attachments: [] } as unknown as FeedbackWorkspaceView
  const call = vi.fn(async () => new ArrayBuffer(4)), create = vi.fn((_blob: Blob) => 'blob:source'), revoke = vi.fn()
  const updateState = vi.fn(), setBusy = vi.fn()
  const context: WorkbenchControllerContext = { requestId: 'request', getState: () => state, getWorkspace: () => workspace,
    updateState, isEditable: () => editable, setBusy, runtime: { transport: { call } as unknown as ApplicationTransport } }
  return { controller: createMediaReviewController(context, { create, revoke }), context, call, create, revoke, updateState, setBusy,
    edit: (next: WorkbenchState | null) => state = next, lock: () => editable = false, workspace: () => workspace,
    replace: (next: FeedbackWorkspaceView) => workspace = next }
}
function player() {
  const element = { currentTime: 2.25, pause: vi.fn(), removeAttribute: vi.fn(), load: vi.fn(() => element.currentTime = 0) }
  return element as unknown as HTMLMediaElement & typeof element
}
describe('request-owned media lifecycle', () => {
  it('loads readonly material once, shares concurrent reads and reuses its Blob across view mounts', async () => {
    const f = fixture(); f.lock()
    let finish!: (value: ArrayBuffer) => void
    f.call.mockImplementationOnce(() => new Promise((resolve) => finish = resolve))
    const first = f.controller.loadSource(data), second = f.controller.loadSource(data)
    expect(first).toBe(second)
    finish(new ArrayBuffer(4)); expect(await first).toEqual({ url: 'blob:source', mediaType: 'video/webm' })
    const old = f.controller.bindPlayer(player()); old.release(); f.controller.bindPlayer(player())
    await f.controller.loadSource(data)
    expect(f.call).toHaveBeenCalledExactlyOnceWith('readRequestAttachment', { request_id: 'request', attachment_id: 'source' })
    expect(f.create).toHaveBeenCalledOnce(); expect(f.create.mock.calls[0][0].type).toBe('video/webm')
    expect(f.updateState).not.toHaveBeenCalled(); expect(f.setBusy).not.toHaveBeenCalled()
    f.controller.dispose(); f.controller.dispose(); expect(f.revoke).toHaveBeenCalledExactlyOnceWith('blob:source')
  })
  it('pauses and detaches an old player before binding another without losing position on late release', () => {
    const f = fixture(), oldPlayer = player(), nextPlayer = player()
    const old = f.controller.bindPlayer(oldPlayer), next = f.controller.bindPlayer(nextPlayer)
    expect(old.active()).toBe(false); expect(next.active()).toBe(true)
    expect(oldPlayer.pause).toHaveBeenCalledOnce(); expect(oldPlayer.removeAttribute).toHaveBeenCalledWith('src')
    expect(f.controller.position()).toBe(2250)
    old.release(); expect(f.controller.position()).toBe(2250)
    nextPlayer.currentTime = 4; next.release(); expect(f.controller.position()).toBe(4000)
  })
  it('rejects ambiguous, missing, mismatched-kind or changed source identity before reading bytes', async () => {
    for (const mode of ['duplicate', 'missing', 'kind', 'request', 'version'] as const) {
      const f = fixture(), workspace = f.workspace(), attachment = workspace.request_attachments[0]
      if (mode === 'duplicate') f.replace({ ...workspace, request_attachments: [attachment, { ...attachment, attachment_id: 'another' }] })
      if (mode === 'missing') f.replace({ ...workspace, request_attachments: [] })
      if (mode === 'kind') f.replace({ ...workspace, request_attachments: [{ ...attachment, media_type: 'audio/webm' }] })
      if (mode === 'request') f.replace({ ...workspace, request: { ...workspace.request, request_id: 'different' } })
      if (mode === 'version') f.replace({ ...workspace, workbench: { ...workspace.workbench!, version: 2 } })
      await expect(f.controller.loadSource(data)).rejects.toThrow(/unavailable|no longer available/)
      expect(f.call).not.toHaveBeenCalled(); expect(f.create).not.toHaveBeenCalled()
    }
  })
  it('does not create a URL for late bytes after disposal and refuses subsequent loads or binds', async () => {
    const f = fixture(); let finish!: (value: ArrayBuffer) => void
    f.call.mockImplementationOnce(() => new Promise((resolve) => finish = resolve))
    const loading = f.controller.loadSource(data); f.controller.dispose(); finish(new ArrayBuffer(4))
    await expect(loading).rejects.toThrow('no longer'); expect(f.create).not.toHaveBeenCalled()
    await expect(f.controller.loadSource(data)).rejects.toThrow('no longer')
    expect(() => f.controller.bindPlayer(player())).toThrow('no longer')
  })
  it('retries a failed read and rejects empty or over-budget bytes before URL allocation', async () => {
    const f = fixture(); f.call.mockRejectedValueOnce(new Error('Read failed'))
    await expect(f.controller.loadSource(data)).rejects.toThrow('Read failed')
    await f.controller.loadSource(data); expect(f.call).toHaveBeenCalledTimes(2)
    for (const length of [0, 20 * 1024 * 1024 + 1]) {
      const invalid = fixture(); invalid.call.mockResolvedValueOnce(new ArrayBuffer(length))
      await expect(invalid.controller.loadSource(data)).rejects.toThrow('20 MiB'); expect(invalid.create).not.toHaveBeenCalled()
    }
  })
  it('pauses submission and cancellation, allows notes-only, and never rewrites state or starts playback', async () => {
    const f = fixture(), element = player(); f.controller.bindPlayer(element)
    await f.controller.prepareSubmission(); expect(element.pause).toHaveBeenCalledOnce()
    f.edit({ type: 'media_review', comments: [{ id: 'blank', start_ms: 1000, end_ms: null, body: '' }] })
    await expect(f.controller.prepareSubmission()).rejects.toThrow('Write a comment')
    f.lock(); await f.controller.prepareSubmission('cancel')
    await expect(f.controller.prepareSubmission()).rejects.toThrow('editable')
    expect(f.call).not.toHaveBeenCalled(); expect(f.updateState).not.toHaveBeenCalled()
  })
})

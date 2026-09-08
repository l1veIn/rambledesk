import { describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({ readApplicationSnapshot: vi.fn() }))
vi.mock('../application/readApplicationSnapshot', () => ({
  readApplicationSnapshot: mocks.readApplicationSnapshot,
}))

import { previewFixtures } from '../previewFixtures'
import { requestTaskViewDescriptor, sessionViewDescriptor } from '../workspace/viewDescriptors'
import {
  createDraftOperationsController,
  type DraftOperationsContext,
} from './draftOperationsController'

const workspace = previewFixtures.workspace
const request = workspace.request

function harness(overrides: Partial<DraftOperationsContext> = {}) {
  const context = {
    transport: { call: vi.fn(async () => null) },
    tr: (source: string) => source,
    messageFrom: (cause: unknown) => String(cause),
    isPreviewMode: () => false,
    getActiveView: () => null,
    getPendingViewKey: () => null,
    getWorkspace: () => null,
    getCurrentRequest: () => null,
    getEditor: () => undefined,
    isWorkbenchMounted: () => false,
    isTransitionLocked: () => false,
    getDraftMessage: () => '',
    saveDraftNow: vi.fn(async () => true),
    setWorkspaceDraft: vi.fn(),
    adoptDraft: vi.fn(),
    setPageError: vi.fn(),
    ...overrides,
  } as unknown as DraftOperationsContext
  return { controller: createDraftOperationsController(context), context }
}

describe('draft operations controller', () => {
  it('applies a foreground operation through the mounted editor and saves', async () => {
    const applyDraftOperation = vi.fn(() => true)
    const saveDraftNow = vi.fn(async () => true)
    const { controller, context } = harness({
      getActiveView: () => sessionViewDescriptor(request.host_id, request.host_session_id),
      isWorkbenchMounted: () => true,
      getWorkspace: () => workspace,
      getEditor: () => ({ applyDraftOperation } as never),
      saveDraftNow,
    })

    await controller.routeDraftOperation(request.request_id, {
      kind: 'clearActionGroup',
      actionId: 'a',
    })

    expect(applyDraftOperation).toHaveBeenCalledWith({ kind: 'clearActionGroup', actionId: 'a' })
    expect(saveDraftNow).toHaveBeenCalled()
    expect(context.setPageError).not.toHaveBeenCalled()
  })

  it('refuses to write through the editor of a closed request', async () => {
    const { controller, context } = harness({
      getActiveView: () => sessionViewDescriptor(request.host_id, request.host_session_id),
      isWorkbenchMounted: () => true,
      getWorkspace: () => ({
        ...workspace,
        request: { ...request, status: 'completed' },
      }),
      getEditor: () => ({ applyDraftOperation: vi.fn(() => true) } as never),
    })

    await expect(
      controller.routeDraftOperation(request.request_id, { kind: 'clearActionGroup', actionId: 'a' }),
    ).rejects.toThrow('read-only')
    expect(context.setPageError).toHaveBeenCalled()
  })

  it('writes a background operation and adopts the saved draft for an active task view', async () => {
    mocks.readApplicationSnapshot.mockResolvedValue(workspace)
    const call = vi.fn(async (_name: string, input: Record<string, unknown>) => ({
      document_json: input.document_json,
      body_markdown: input.body_markdown,
      saved_revision: Number(input.expected_revision) + 1,
      updated_at: null,
    }))
    const { controller, context } = harness({
      transport: { call } as never,
      getActiveView: () => requestTaskViewDescriptor(request.request_id),
      getCurrentRequest: () => request,
      getWorkspace: () => workspace,
    })

    await controller.routeDraftOperation(request.request_id, {
      kind: 'appendClipboardText',
      text: 'hello',
      label: 'Clipboard',
      action: null,
    })

    expect(call).toHaveBeenCalledWith(
      'saveFeedbackDraft',
      expect.objectContaining({ request_id: request.request_id }),
    )
    expect(context.setWorkspaceDraft).toHaveBeenCalled()
    expect(context.adoptDraft).toHaveBeenCalled()
  })

  it('reports a failed background write and rethrows', async () => {
    mocks.readApplicationSnapshot.mockRejectedValue(new Error('offline'))
    const { controller, context } = harness()

    await expect(
      controller.routeDraftOperation(request.request_id, { kind: 'clearActionGroup', actionId: 'a' }),
    ).rejects.toThrow('offline')
    expect(context.setPageError).toHaveBeenCalledWith('Failed to write Ramble content: {error}')
  })

  it('serializes queued document tasks', async () => {
    const order: string[] = []
    const { controller } = harness()
    const first = controller.enqueueDocumentTask(async () => {
      await Promise.resolve()
      order.push('first')
    })
    const second = controller.enqueueDocumentTask(async () => {
      order.push('second')
    })
    await Promise.all([first, second])
    expect(order).toEqual(['first', 'second'])
  })

  it('toggles the active action group for the open request', () => {
    const { controller } = harness({ getCurrentRequest: () => request })
    controller.selectAction('action-1', 0, 'Action one')
    expect(controller.activeActionId(request.request_id)).toBe('action-1')
    expect(controller.activeActionFor(request.request_id)).toEqual({
      actionId: 'action-1',
      actionIndex: 0,
      title: 'Action one',
    })
    controller.selectAction('action-1', 0, 'Action one')
    expect(controller.activeActionId(request.request_id)).toBeNull()
  })

  it('ignores a selection while a view transition is locked', () => {
    const { controller } = harness({
      getCurrentRequest: () => request,
      isTransitionLocked: () => true,
    })
    controller.selectAction('action-1', 0, 'Action one')
    expect(controller.activeActionFor(request.request_id)).toBeNull()
  })
})

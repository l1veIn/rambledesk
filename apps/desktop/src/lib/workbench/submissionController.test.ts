import { get } from 'svelte/store'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { ApplicationTransport } from '../application/applicationTransport'
import type { PublishedFeedbackAction } from '../publishedFeedbackAction'
import { previewFixtures } from '../previewFixtures'
import { createDraftSession } from './draftSession'
import { createSubmissionController } from './submissionController'
import { createWorkspaceSession } from './workspaceSession'

afterEach(() => {
  vi.unstubAllGlobals()
})

function harness(overrides: {
  call?: ApplicationTransport['call']
  canCancel?: boolean
  rambleCanExit?: boolean
  action?: PublishedFeedbackAction
} = {}) {
  vi.stubGlobal('window', { confirm: () => true })
  const session = createWorkspaceSession()
  const draftSession = createDraftSession()
  session.open({
    ...previewFixtures.workspace,
    request: { ...previewFixtures.workspace.request, allow_finish: true },
  })
  const pageErrors: string[] = []
  const notifications: string[] = []
  const refreshes = { count: 0 }
  const exitRamble = vi.fn(async () => {})
  const controller = createSubmissionController({
    transport: {
      call: overrides.call ?? vi.fn(async () => ({
        ...previewFixtures.workspace.request,
        status: 'completed' as const,
        resolution: 'approved' as const,
      })),
      subscribe: () => () => {},
      waitUntilReady: async () => {},
      capabilities: () => previewFixtures.workspace.request as never,
    } as unknown as ApplicationTransport,
    session,
    draftSession,
    publishedFeedbackAction: overrides.action ?? {
      label: 'Open feedback package',
      run: vi.fn(async () => {}),
    },
    tr: (source) => source,
    messageFrom: (cause) => (cause instanceof Error ? cause.message : String(cause)),
    canCancel: () => overrides.canCancel ?? true,
    rambleCanExit: () => overrides.rambleCanExit ?? false,
    exitRamble,
    refreshNavigation: async () => {
      refreshes.count += 1
    },
    setPageError: (message) => pageErrors.push(message),
    notifyApproved: () => notifications.push('approved'),
    notifyCancelled: () => notifications.push('cancelled'),
  })
  return { controller, session, draftSession, pageErrors, notifications, refreshes, exitRamble }
}

describe('submission controller', () => {
  it('approves through the transport and folds the result into the session', async () => {
    const calls: Array<{ name: string; input: unknown }> = []
    const { controller, session, notifications, refreshes } = harness({
      call: (async (name: string, input: unknown) => {
        calls.push({ name, input })
        return { ...previewFixtures.workspace.request, status: 'completed', resolution: 'approved' }
      }) as unknown as ApplicationTransport['call'],
    })

    await controller.approveFeedback()

    expect(calls).toEqual([
      { name: 'approveFeedbackRequest', input: { request_id: previewFixtures.workspace.request.request_id } },
    ])
    expect(session.request()?.status).toBe('completed')
    expect(notifications).toEqual(['approved'])
    expect(refreshes.count).toBe(1)
    expect(get(session).approving).toBe(false)
  })

  it('reports an approval failure without touching the request', async () => {
    const { controller, session, pageErrors } = harness({
      call: (async () => {
        throw new Error('server down')
      }) as unknown as ApplicationTransport['call'],
    })

    await controller.approveFeedback()

    expect(pageErrors.at(-1)).toBe('server down')
    expect(session.request()?.status).toBe(previewFixtures.workspace.request.status)
    expect(get(session).approving).toBe(false)
  })

  it('cancels, marks the draft saved and notifies', async () => {
    const { controller, session, draftSession, notifications } = harness({
      call: (async () => ({
        ...previewFixtures.workspace.request,
        status: 'cancelled',
        resolution: null,
      })) as unknown as ApplicationTransport['call'],
    })
    draftSession.edit({ documentJson: '{"type":"doc"}', bodyMarkdown: 'pending' })
    expect(get(draftSession).phase).toBe('unsaved')

    await controller.cancelFeedback()

    expect(session.request()?.status).toBe('cancelled')
    expect(get(draftSession).phase).toBe('saved')
    expect(notifications).toEqual(['cancelled'])
    expect(get(session).cancelling).toBe(false)
  })

  it('ignores cancel when the request cannot be cancelled', async () => {
    const { controller, session } = harness({ canCancel: false })

    await controller.cancelFeedback()

    expect(session.request()?.status).toBe(previewFixtures.workspace.request.status)
    expect(get(session).cancelling).toBe(false)
  })

  it('delegates package opening and reports its failure', async () => {
    const run = vi.fn(async () => {
      throw new Error('missing package')
    })
    const { controller, session, pageErrors } = harness({
      action: { label: 'Open feedback package', run },
    })
    session.setCompleted({
      ...previewFixtures.workspace.request,
      execution_mode: 'poll',
      status: 'completed',
      feedback: { markdown: 'package' } as never,
      resolution: 'approved',
    })

    await controller.openFeedbackPackage()

    expect(run).toHaveBeenCalledWith(previewFixtures.workspace.request.request_id)
    expect(pageErrors).toEqual(['Could not open Feedback Package: {error}'])
  })
})

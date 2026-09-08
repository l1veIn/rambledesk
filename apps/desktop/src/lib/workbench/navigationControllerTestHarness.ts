import { vi } from 'vitest'

import type {
  FeedbackRequestSummary,
  HostSessionSummary,
} from '../feedback'
import { TestApplicationTransport } from '../application/testApplicationTransport'
import { createUnavailableWorkbenchCapabilities } from '../capabilities/unavailableCapabilities'
import { createNavigationController } from './navigationController'

const unavailableCapabilities = createUnavailableWorkbenchCapabilities()

export const mocks = {
  applicationCall: vi.fn(),
  notificationSend: vi.fn(),
  setPendingCount: vi.fn(),
}

export function resetNavigationTestMocks() {
  mocks.applicationCall.mockReset()
  mocks.notificationSend.mockReset()
  mocks.notificationSend.mockResolvedValue(undefined)
  mocks.setPendingCount.mockReset()
  mocks.setPendingCount.mockResolvedValue(undefined)
}

export function testCapabilities() {
  return {
    notifications: {
      status: { availability: 'available', source: 'native' } as const,
      implementation: {
        ...unavailableCapabilities.notifications.implementation,
        send: mocks.notificationSend,
      },
    },
    tray: {
      status: { availability: 'available', source: 'native' } as const,
      implementation: { setPendingCount: mocks.setPendingCount },
    },
  }
}

export function feedbackRequest(requestId: string): FeedbackRequestSummary {
  return {
    request_id: requestId,
    host_id: 'codex',
    host_session_id: 'session-1',
    source_hint: 'Workbench',
    title: 'Review refresh behavior',
    what_happened: 'The refresh button should update page data.',
    status: 'waiting',
    resolution: null,
    allow_finish: false,
    final_summary: null,
    revision: 1,
    created_at: '2026-08-22T00:00:00Z',
    updated_at: '2026-08-22T00:01:00Z',
  }
}

export function hostSession(overrides: Partial<HostSessionSummary> = {}): HostSessionSummary {
  return {
    session_id: 'local-session-1',
    management: { kind: 'external' },
    host_id: 'codex',
    host_session_id: 'session-1',
    title: 'Refresh workbench',
    source_hint: 'Workbench',
    request_count: 1,
    pending_count: 1,
    updated_at: '2026-08-22T00:01:00Z',
    pinned_at: null,
    archived_at: null,
    host_pinned_at: null,
    ...overrides,
  }
}

export function createController(
  overrides: Partial<Parameters<typeof createNavigationController>[0]> = {},
) {
  const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
    .handle('listFeedbackInbox', (input) => mocks.applicationCall('listFeedbackInbox', input))
    .handle('listHostSessions', (input) => mocks.applicationCall('listHostSessions', input))
    .handle('listHostProfiles', (input) => mocks.applicationCall('listHostProfiles', input))
    .handle('listFeedbackRequests', (input) => mocks.applicationCall('listFeedbackRequests', input))
    .handle('renameHostSession', (input) => mocks.applicationCall('renameHostSession', input))
    .handle('setHostSessionPinned', (input) => mocks.applicationCall('setHostSessionPinned', input))
    .handle('archiveHostSession', (input) => mocks.applicationCall('archiveHostSession', input))
    .handle('setHostPinned', (input) => mocks.applicationCall('setHostPinned', input))
  return createNavigationController({
    capabilities: testCapabilities(),
    previewMode: false,
    transport,
    tr: (source) => source,
    messageFrom: (cause) => String(cause),
    getNotificationState: () => 'disabled',
    getWorkspaceRequestId: () => undefined,
    isDirty: () => false,
    saveDraftNow: vi.fn(async () => true),
    openRequest: vi.fn(async () => true),
    clearWorkspace: vi.fn(),
    onPageError: vi.fn(),
    canSendOsBanners: () => false,
    ...overrides,
  })
}

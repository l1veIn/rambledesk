import type {
  FeedbackRequestSummary,
  FeedbackWorkspaceView,
  HostSessionSummary,
} from './feedback'
import type { HostProfile, ResumePrompt } from './workbench/types'

const requests: FeedbackRequestSummary[] = [
  {
    request_id: '019fc1d9-51e7-7eb2-b196-e9266947fc41',
    host_id: 'codex',
    host_session_id: 'desktop-refactor-2026-08-02',
    source_hint: 'Desktop workbench',
    title: 'Review host and session navigation',
    what_happened: 'The Inbox now groups requests by host and host session.',
    status: 'in_progress',
    resolution: null,
    allow_finish: false,
    final_summary: null,
    revision: 3,
    created_at: '2026-08-02T06:30:00Z',
    updated_at: '2026-08-02T08:12:00Z',
  },
  {
    request_id: '019fc1d9-51e7-7eb2-b196-e9266947fc42',
    host_id: 'codex',
    host_session_id: 'desktop-refactor-2026-08-02',
    source_hint: 'Settings / Adapters',
    title: 'Inspect adapter settings',
    what_happened: 'Settings were reorganized around complete host adapters.',
    status: 'waiting',
    resolution: null,
    allow_finish: false,
    final_summary: null,
    revision: 0,
    created_at: '2026-08-02T07:15:00Z',
    updated_at: '2026-08-02T07:15:00Z',
  },
  {
    request_id: '019fc1d9-51e7-7eb2-b196-e9266947fc43',
    host_id: 'pi',
    host_session_id: 'pi-native-wait',
    source_hint: 'Pi native adapter',
    title: 'Verify native wait flow',
    what_happened: 'Pi now waits through the local JSON API in the active tool call.',
    status: 'completed',
    resolution: 'feedback_submitted',
    allow_finish: false,
    final_summary: null,
    revision: 4,
    created_at: '2026-08-02T05:40:00Z',
    updated_at: '2026-08-02T06:05:00Z',
  },
  {
    request_id: '019fc1d9-51e7-7eb2-b196-e9266947fc44',
    host_id: 'claude',
    host_session_id: 'terminology-audit',
    source_hint: 'Protocol review',
    title: 'Check terminology residuals',
    what_happened: 'The current tree was scanned for removed protocol vocabulary.',
    status: 'cancelled',
    resolution: 'cancelled',
    allow_finish: false,
    final_summary: null,
    revision: 1,
    created_at: '2026-08-02T04:10:00Z',
    updated_at: '2026-08-02T04:35:00Z',
  },
]

const hostSessions: HostSessionSummary[] = [
  {
    host_id: 'codex',
    host_session_id: 'desktop-refactor-2026-08-02',
    title: 'Review host and session navigation',
    source_hint: 'C:/workspace/rambledesk',
    session_id: 'preview-codex-session',
    management: { kind: 'external' },
    request_count: 2,
    pending_count: 2,
    updated_at: '2026-08-02T08:12:00Z',
    pinned_at: '2026-08-02T08:20:00Z',
    archived_at: null,
    host_pinned_at: '2026-08-02T08:21:00Z',
  },
  {
    host_id: 'pi',
    host_session_id: 'pi-native-wait',
    title: 'Verify native wait flow',
    source_hint: 'C:/workspace/pi-rambledesk',
    session_id: 'preview-pi-session',
    management: { kind: 'external' },
    request_count: 1,
    pending_count: 0,
    updated_at: '2026-08-02T06:05:00Z',
    pinned_at: null,
    archived_at: null,
    host_pinned_at: null,
  },
  {
    host_id: 'claude',
    host_session_id: 'terminology-audit',
    title: 'Check terminology residuals',
    source_hint: 'C:/workspace/protocol',
    session_id: 'preview-claude-session',
    management: { kind: 'external' },
    request_count: 1,
    pending_count: 0,
    updated_at: '2026-08-02T04:35:00Z',
    pinned_at: null,
    archived_at: null,
    host_pinned_at: null,
  },
]

const archivedHostSessions: HostSessionSummary[] = [
  {
    host_id: 'codex',
    host_session_id: 'archived-preview-session',
    title: 'Archived workspace recovery preview',
    source_hint: 'C:/workspace/archived-preview',
    session_id: 'preview-archived-session',
    management: { kind: 'external' },
    request_count: 0,
    pending_count: 0,
    updated_at: '2026-08-01T08:12:00Z',
    pinned_at: null,
    archived_at: '2026-08-02T08:12:00Z',
    host_pinned_at: null,
  },
]

const hostProfiles: HostProfile[] = [
  {
    id: 'codex',
    label: 'Codex',
    icon_svg: '',
    default_adapter: 'generic_mcp',
    continuation_mode: 'manual',
  },
  {
    id: 'pi',
    label: 'Pi',
    icon_svg: '',
    default_adapter: 'pi_native',
    continuation_mode: 'not_required',
  },
  {
    id: 'claude',
    label: 'Claude Code',
    icon_svg: '',
    default_adapter: 'generic_mcp',
    continuation_mode: 'manual',
  },
]

const workspace: FeedbackWorkspaceView = {
  request: requests[0],
  actions: [
    {
      id: 'host-session-filter',
      instruction: 'Switch between Codex, Pi, and Claude host groups.',
    },
    {
      id: 'request-list',
      instruction: 'Confirm terminal and pending requests share one chronological list.',
    },
    {
      id: 'responsive-workspace',
      instruction: 'Resize the window and verify the editor remains usable.',
    },
  ],
  context_refs: [
    {
      label: 'Terminology',
      uri: 'file:///Users/yangchen/Desktop/rambledesk/docs/TERMINOLOGY.md',
    },
  ],
  request_attachments: [
    {
      attachment_id: '019fc1d9-51e7-7eb2-b196-e9266947fc51',
      file_name: 'review-notes.md',
      media_type: 'text/markdown',
      byte_size: 1548,
      sha256: 'preview-markdown',
      position: 0,
    },
    {
      attachment_id: '019fc1d9-51e7-7eb2-b196-e9266947fc52',
      file_name: 'navigation-mockup.png',
      media_type: 'image/png',
      byte_size: 245760,
      sha256: 'preview-image',
      position: 1,
    },
  ],
  draft: {
    document_json: null,
    body_markdown:
      'The host and session hierarchy is easy to scan.\n\nThe request list keeps terminal work in the same chronological view.',
    saved_revision: 3,
    updated_at: '2026-08-02T08:14:00Z',
  },
  attachments: [],
  feedback: null,
}

const resumePrompt: ResumePrompt = {
  request_id: requests[0].request_id,
  host_id: 'codex',
  host_label: 'Codex',
  title: 'Return to Codex to continue',
  body: 'The feedback package is ready. Continue the original host session with the prompt below.',
  resume_prompt:
    'RambleDesk feedback request 019fc1d9-51e7-7eb2-b196-e9266947fc41 is completed. Call get_feedback with this request_id, verify the package, and continue the original task.',
  reason: 'completed',
}

export const previewFixtures = {
  requests,
  hostSessions,
  archivedHostSessions,
  hostProfiles,
  workspace,
  resumePrompt,
}

export function previewWorkspaceFor(requestId: string): FeedbackWorkspaceView | null {
  const request = requests.find((candidate) => candidate.request_id === requestId)
  if (!request) return null

  return {
    ...workspace,
    request,
    draft: {
      ...workspace.draft,
      body_markdown:
        request.request_id === workspace.request.request_id
          ? workspace.draft.body_markdown
          : `Feedback notes for ${request.title}.`,
    },
    feedback: null,
  }
}

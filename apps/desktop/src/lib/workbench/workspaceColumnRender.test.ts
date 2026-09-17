import { createRawSnippet } from 'svelte'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'svelte/server'

import type { HostProfile } from '$lib/domain/hostProfile'
import { previewFixtures } from '$lib/preview/previewFixtures'
import FeedbackColumn from './FeedbackColumn.svelte'
import WorkbenchContainer from './WorkbenchContainer.svelte'
import WorkspaceHeader from './WorkspaceHeader.svelte'

vi.mock('$lib/preferences', async () => {
  const { writable } = await import('svelte/store')
  return { locale: writable('en'), distinguishUntidiedText: writable(false) }
})

const workspace = previewFixtures.workspace
const resolveHostProfile = (hostId: string) => ({
  id: hostId,
  label: 'Codex',
  icon_svg: '<svg></svg>',
}) as unknown as HostProfile

const agentStatus = createRawSnippet(() => ({ render: () => '<span data-stub-acp>ACP</span>' }))

describe('workbench column identity', () => {
  it('carries the full request identity, including the title the tab strip used to repeat', () => {
    const body = render(WorkspaceHeader, { props: { workspace, resolveHostProfile, cooking: false } }).body
    expect(body).toContain('Codex')
    expect(body).toContain(workspace.request.host_session_id)
    expect(body).toContain(workspace.request.title)
    expect(body).toContain('<h1')
  })
})

describe('feedback column identity', () => {
  it('carries the Agent status at the top of the feedback column', () => {
    const body = render(FeedbackColumn, { props: {
      workspace,
      formatTime: () => '10:00',
      agentStatus,
    } }).body
    expect(body).toContain('data-feedback-agent-status')
    expect(body).toContain('data-stub-acp')
    // Status first, then the document title with its submit row, in one column.
    expect(body.indexOf('data-feedback-agent-status')).toBeLessThan(body.indexOf('data-feedback-actions'))
    expect(body.indexOf('data-stub-acp')).toBeLessThan(body.indexOf('Submit feedback'))
  })

  it('keeps submit on the document title row and out of a bottom bar', () => {
    const body = render(FeedbackColumn, { props: { workspace, formatTime: () => '10:00', canSubmit: true } }).body
    const actions = body.indexOf('data-feedback-actions')
    expect(actions).toBeGreaterThan(-1)
    // The action row sits with the Feedback document heading, above the editor body.
    expect(body.indexOf('Feedback document')).toBeLessThan(actions)
    expect(actions).toBeLessThan(body.indexOf('editor-host'))
    // Cancel is icon-only: its label lives in the accessible name, not in text.
    expect(body).toContain('aria-label="Cancel feedback"')
    expect(body).not.toContain('>Cancel feedback<')
  })

  it('renders no agent status row without one', () => {
    const body = render(FeedbackColumn, { props: { workspace, formatTime: () => '10:00' } }).body
    expect(body).not.toContain('data-feedback-agent-status')
    expect(body).toContain('Submit feedback')
  })
})

describe('workbench container composition', () => {
  const workbench = createRawSnippet(() => ({ render: () => '<div data-workbench="stub">Brief</div>' }))
  const unavailable = {
    serverPaths: { status: { availability: 'unavailable', source: 'none' }, implementation: {} },
    imagePaste: { status: { availability: 'unavailable', source: 'none' }, implementation: {} },
  } as never

  it('keeps the Agent status inside the feedback column, never above the workbench', () => {
    const body = render(WorkbenchContainer, { props: {
      workspace,
      transport: {} as never,
      capabilities: unavailable,
      formatTime: () => '10:00',
      workbench,
      agentStatus,
    } }).body

    const feedbackPane = body.indexOf('data-pane-id="feedback-column-pane"')
    const workbenchPane = body.indexOf('data-pane-id="workbench-pane"')
    const status = body.indexOf('data-feedback-agent-status')

    expect(workbenchPane).toBeGreaterThan(-1)
    expect(feedbackPane).toBeGreaterThan(workbenchPane)
    // The status row belongs to the feedback pane that follows the workbench pane.
    expect(status).toBeGreaterThan(feedbackPane)
    expect(body).toContain('data-workbench="stub"')
    expect(body.indexOf('data-workbench="stub"')).toBeLessThan(status)
  })
})

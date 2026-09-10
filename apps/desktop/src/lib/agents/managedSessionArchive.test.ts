import { describe, expect, it } from 'vitest'
import {
  agentDraftViewDescriptor,
  agentSessionViewDescriptor,
  sessionViewDescriptor,
  workspaceViewKey,
} from '$lib/workspace/viewDescriptors'
import { removeArchivedManagedSessionView } from './managedSessionArchive'

describe('managed session archive navigation', () => {
  const archived = agentSessionViewDescriptor('archived')
  const other = agentSessionViewDescriptor('other')
  const archivedKey = workspaceViewKey(archived)
  const otherKey = workspaceViewKey(other)

  it('removes the archived tab without taking over a pending switch to another view', () => {
    const shell = { views: [archived, other], activeViewKey: archivedKey }
    const result = removeArchivedManagedSessionView(shell, 'archived', otherKey)

    expect(result).toEqual({
      shell: { views: [other], activeViewKey: otherKey },
      shouldNavigateToArchive: false,
      shouldInvalidatePending: false,
    })
    expect(shell.views).toEqual([archived, other])
  })

  it('keeps the current view when the user already switched away', () => {
    const result = removeArchivedManagedSessionView(
      { views: [archived, other], activeViewKey: otherKey }, 'archived', null,
    )

    expect(result.shell).toEqual({ views: [other], activeViewKey: otherKey })
    expect(result.shouldNavigateToArchive).toBe(false)
    expect(result.shouldInvalidatePending).toBe(false)
  })

  it('cancels a pending activation of the archived session without redirecting another active view', () => {
    const result = removeArchivedManagedSessionView(
      { views: [archived, other], activeViewKey: otherKey }, 'archived', archivedKey,
    )

    expect(result.shell).toEqual({ views: [other], activeViewKey: otherKey })
    expect(result.shouldNavigateToArchive).toBe(false)
    expect(result.shouldInvalidatePending).toBe(true)
  })

  it.each([null, archivedKey])('redirects the active archived session when pending target is %s', (pending) => {
    const result = removeArchivedManagedSessionView(
      { views: [archived], activeViewKey: archivedKey }, 'archived', pending,
    )

    expect(result.shell).toEqual({ views: [], activeViewKey: null })
    expect(result.shouldNavigateToArchive).toBe(true)
    expect(result.shouldInvalidatePending).toBe(pending === archivedKey)
  })

  it('preserves draft and feedback tabs while removing only the archived conversation', () => {
    const draft = agentDraftViewDescriptor('draft')
    const feedback = sessionViewDescriptor('claude-code', 'host-session')
    const result = removeArchivedManagedSessionView(
      { views: [draft, archived, feedback], activeViewKey: workspaceViewKey(draft) }, 'archived', null,
    )

    expect(result.shell.views).toEqual([draft, feedback])
    expect(result.shell.activeViewKey).toBe(workspaceViewKey(draft))
  })

  it('cancels a pending archived target even when it has not yet opened a tab', () => {
    const shell = { views: [other], activeViewKey: otherKey }
    const result = removeArchivedManagedSessionView(shell, 'archived', archivedKey)

    expect(result.shell).toBe(shell)
    expect(result.shouldNavigateToArchive).toBe(false)
    expect(result.shouldInvalidatePending).toBe(true)
  })
})

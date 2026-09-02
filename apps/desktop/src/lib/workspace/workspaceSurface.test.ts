import { describe, expect, it } from 'vitest'

import {
  archiveViewDescriptor,
  inboxViewDescriptor,
  rambelleProfileViewDescriptor,
  requestTaskViewDescriptor,
  sessionViewDescriptor,
  settingsViewDescriptor,
} from './viewDescriptors'
import { workspaceSurface } from './workspaceSurface'

describe('workspaceSurface', () => {
  it('keeps the request list inside each Session tab surface', () => {
    expect(workspaceSurface(sessionViewDescriptor('codex', 'session-1'))).toBe('session')
  })

  it('gives the singleton Inbox tab the aggregate request list surface', () => {
    expect(workspaceSurface(inboxViewDescriptor())).toBe('inbox')
  })

  it('lets non-Session tabs use the complete surface beside the Session rail', () => {
    expect(workspaceSurface(archiveViewDescriptor())).toBe('standalone')
    expect(workspaceSurface(settingsViewDescriptor())).toBe('standalone')
    expect(workspaceSurface(requestTaskViewDescriptor('request-1'))).toBe('standalone')
    expect(workspaceSurface(rambelleProfileViewDescriptor())).toBe('standalone')
    expect(workspaceSurface(null)).toBe('standalone')
  })
})

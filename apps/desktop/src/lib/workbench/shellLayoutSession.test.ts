import { get } from 'svelte/store'
import { describe, expect, it } from 'vitest'

import { createShellLayoutSession } from './shellLayoutSession'

describe('shell layout session', () => {
  it('follows the stored rail preference on wide viewports', () => {
    const session = createShellLayoutSession()
    session.setRailCollapsed('host', true)
    expect(get(session).hostCollapsed).toBe(true)
    session.setRailCollapsed('host', false)
    expect(get(session).hostCollapsed).toBe(false)
  })

  it('treats collapse as drawer state on phones', () => {
    const session = createShellLayoutSession()
    session.setMode('phone')
    expect(get(session).hostCollapsed).toBe(true)
    session.setRailCollapsed('host', false)
    expect(get(session).hostCollapsed).toBe(false)
    expect(get(session).requestCollapsed).toBe(true)
  })

  it('opens only one phone drawer at a time', () => {
    const session = createShellLayoutSession()
    session.setMode('phone')
    session.setRailCollapsed('host', false)
    session.setRailCollapsed('request', false)
    expect(get(session).hostCollapsed).toBe(true)
    expect(get(session).requestCollapsed).toBe(false)
  })

  it('keeps the desktop preference while a phone drawer is used', () => {
    const session = createShellLayoutSession()
    session.setMode('phone')
    session.setRailCollapsed('host', false)
    session.setMode('desktop')
    expect(get(session).hostCollapsed).toBe(false)
    expect(get(session).mode).toBe('desktop')
  })

  it('closes both drawers only on phones', () => {
    const session = createShellLayoutSession()
    session.setMode('phone')
    session.setRailCollapsed('host', false)
    session.closePhoneDrawers()
    expect(get(session).hostCollapsed).toBe(true)

    session.setMode('desktop')
    session.setRailCollapsed('request', false)
    session.closePhoneDrawers()
    expect(get(session).requestCollapsed).toBe(false)
  })
})

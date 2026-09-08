import { get, writable } from 'svelte/store'

import {
  initialHostRailCollapsed,
  initialRequestRailCollapsed,
  saveHostRailCollapsed,
  saveRequestRailCollapsed,
} from '../uiPreferences'
import type { ShellMode } from './shellMode'

export type ShellRail = 'host' | 'request'

export type ShellLayoutSessionState = Readonly<{
  mode: ShellMode
  hostCollapsed: boolean
  requestCollapsed: boolean
}>

/**
 * Viewport class and rail presentation. On phones a rail is a drawer, so
 * `collapsed` means "drawer closed" and the persisted desktop preference is left
 * untouched until the viewport widens again.
 */
export function createShellLayoutSession() {
  const internals = {
    mode: 'desktop' as ShellMode,
    hostPreference: initialHostRailCollapsed(),
    requestPreference: initialRequestRailCollapsed(),
    phoneHostOpen: false,
    phoneRequestOpen: false,
  }
  const store = writable(project())

  function project(): ShellLayoutSessionState {
    const phone = internals.mode === 'phone'
    return {
      mode: internals.mode,
      hostCollapsed: phone ? !internals.phoneHostOpen : internals.hostPreference,
      requestCollapsed: phone ? !internals.phoneRequestOpen : internals.requestPreference,
    }
  }

  function publish(persist = false) {
    if (persist) {
      saveHostRailCollapsed(internals.hostPreference)
      saveRequestRailCollapsed(internals.requestPreference)
    }
    const next = project()
    store.update((current) =>
      current.mode === next.mode &&
      current.hostCollapsed === next.hostCollapsed &&
      current.requestCollapsed === next.requestCollapsed
        ? current
        : next,
    )
  }

  function setMode(mode: ShellMode) {
    if (internals.mode === mode) return
    internals.mode = mode
    publish()
  }

  function setRailCollapsed(rail: ShellRail, collapsed: boolean) {
    if (internals.mode === 'phone') {
      if (rail === 'host') {
        internals.phoneHostOpen = !collapsed
        if (!collapsed) internals.phoneRequestOpen = false
      } else {
        internals.phoneRequestOpen = !collapsed
        if (!collapsed) internals.phoneHostOpen = false
      }
      publish()
      return
    }
    if (rail === 'host') internals.hostPreference = collapsed
    else internals.requestPreference = collapsed
    publish(true)
  }

  function closePhoneDrawers() {
    if (internals.mode !== 'phone') return
    internals.phoneHostOpen = false
    internals.phoneRequestOpen = false
    publish()
  }

  return {
    subscribe: store.subscribe,
    setMode,
    setRailCollapsed,
    closePhoneDrawers,
    state: () => get(store),
  }
}

export type ShellLayoutSession = ReturnType<typeof createShellLayoutSession>

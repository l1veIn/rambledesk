import { get } from 'svelte/store'
import { describe, expect, it, vi } from 'vitest'
import { createDiagnosticSettingsController } from './diagnosticSettingsController'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (cause: unknown) => void
  const promise = new Promise<T>((accept, fail) => { resolve = accept; reject = fail })
  return { promise, resolve, reject }
}

function setup() {
  const api = {
    readSettings: vi.fn(async () => ({ enabled: true })),
    setEnabled: vi.fn(async (enabled: boolean) => ({ enabled })),
    clear: vi.fn(async () => {}),
  }
  const setClientEnabled = vi.fn()
  const flush = vi.fn(async () => {})
  const controller = createDiagnosticSettingsController(api, { setClientEnabled, flush })
  return { api, controller, setClientEnabled, flush }
}

describe('diagnostic recording controls', () => {
  it('changes the client recorder only after native acknowledgement, using the returned value', async () => {
    const { controller, api, setClientEnabled } = setup()
    await controller.load()
    setClientEnabled.mockClear()
    const save = deferred<{ enabled: boolean }>()
    api.setEnabled.mockReturnValueOnce(save.promise)
    const saving = controller.setEnabled(false)
    expect(get(controller).enabled).toBe(true)
    expect(setClientEnabled).not.toHaveBeenCalled()
    save.resolve({ enabled: true })
    await saving
    expect(get(controller).enabled).toBe(true)
    expect(setClientEnabled).toHaveBeenCalledExactlyOnceWith(true)
  })

  it('keeps the confirmed setting after a failed save and supports retry', async () => {
    const { controller, api, setClientEnabled } = setup()
    await controller.load()
    api.setEnabled.mockRejectedValueOnce(Error('Disk is read-only'))
    expect(await controller.setEnabled(false)).toBe(false)
    expect(get(controller)).toMatchObject({ enabled: true, busy: null, error: { action: 'saving', message: 'Disk is read-only' } })
    expect(setClientEnabled).toHaveBeenCalledTimes(1)
    expect(await controller.setEnabled(false)).toBe(true)
    expect(get(controller)).toMatchObject({ enabled: false, error: null })
  })

  it('does not invent a default when loading fails, and can reload the persisted choice', async () => {
    const { controller, api, setClientEnabled } = setup()
    api.readSettings.mockRejectedValueOnce(Error('Unavailable'))
    expect(await controller.load()).toBe(false)
    expect(get(controller)).toMatchObject({ enabled: null, error: { action: 'loading', message: 'Unavailable' } })
    expect(setClientEnabled).not.toHaveBeenCalled()
    expect(api.setEnabled).not.toHaveBeenCalled()
    api.readSettings.mockResolvedValueOnce({ enabled: false })
    await controller.load()
    expect(get(controller)).toMatchObject({ enabled: false, error: null })
  })

  it('can explicitly stop recording after an unreadable persisted setting, once native repair succeeds', async () => {
    const { controller, api, setClientEnabled } = setup()
    api.readSettings.mockRejectedValueOnce(Error('Invalid settings JSON'))
    await controller.load()
    const repair = deferred<{ enabled: boolean }>()
    api.setEnabled.mockReturnValueOnce(repair.promise)
    const stopping = controller.setEnabled(false)
    expect(api.setEnabled).toHaveBeenCalledExactlyOnceWith(false)
    expect(get(controller)).toMatchObject({ enabled: null, busy: 'saving' })
    expect(setClientEnabled).not.toHaveBeenCalled()
    repair.resolve({ enabled: false })
    expect(await stopping).toBe(true)
    expect(get(controller)).toMatchObject({ enabled: false, error: null, busy: null })
    expect(setClientEnabled).toHaveBeenCalledExactlyOnceWith(false)
  })

  it('waits for pending frontend writes before clearing without changing recording preference', async () => {
    const { controller, api, flush, setClientEnabled } = setup()
    await controller.load()
    const writes = deferred<void>()
    flush.mockReturnValueOnce(writes.promise)
    const clearing = controller.clear()
    expect(api.clear).not.toHaveBeenCalled()
    expect(get(controller).busy).toBe('clearing')
    expect(await controller.setEnabled(false)).toBe(false)
    expect(await controller.clear()).toBe(false)
    writes.resolve()
    expect(await clearing).toBe(true)
    expect(api.clear).toHaveBeenCalledTimes(1)
    expect(setClientEnabled).toHaveBeenCalledTimes(1)
    expect(get(controller)).toMatchObject({ enabled: true, cleared: true, busy: null })
  })

  it('shows clear failures and permits a successful retry', async () => {
    const { controller, api } = setup()
    await controller.load()
    api.clear.mockRejectedValueOnce(Error('Cannot remove log file'))
    expect(await controller.clear()).toBe(false)
    expect(get(controller)).toMatchObject({ enabled: true, cleared: false, error: { action: 'clearing', message: 'Cannot remove log file' } })
    expect(await controller.clear()).toBe(true)
    expect(get(controller)).toMatchObject({ cleared: true, error: null })
  })
})

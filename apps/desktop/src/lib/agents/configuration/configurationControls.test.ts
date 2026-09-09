import { describe, expect, it, vi } from 'vitest'
import { render } from 'svelte/server'
import type { SessionConfiguration } from '$lib/generated/feedback'
import {
  changeForControl,
  choiceGroups,
  configurationControls,
  controlDisplayValue,
  primaryConfigurationControl,
} from './configurationControls'
import SessionConfigurationControls from './SessionConfigurationControls.svelte'

vi.mock('$lib/preferences', async () => {
  const { writable } = await import('svelte/store')
  return { locale: writable('en') }
})

function configuration(): SessionConfiguration {
  return { options: [
    { id: 'model-picker', category: 'model', name: 'Agent model', description: 'Select a model', kind: { type: 'select', current_value: 'provider/model-a', options: [
      { value: 'provider/model-a', name: 'A', description: 'First model', group: 'Provider' },
      { value: 'provider/model-b', name: 'B', description: 'Second model', group: 'Provider' },
    ] } },
    { id: 'auto', category: null, name: 'Auto approve', description: null, kind: { type: 'boolean', current_value: false } },
    { id: 'opaque-mode', category: 'mode', name: 'Mode', description: null, kind: { type: 'select', current_value: 'ask', options: [
      { value: 'ask', name: 'Ask', description: null, group: null },
      { value: 'edit', name: 'Edit', description: null, group: null },
    ] } },
  ] }
}

describe('Agent-confirmed session controls', () => {
  it('renders supplied options uniformly and updates when the advertisement changes', () => {
    const config = configuration()
    const controls = configurationControls(config)
    expect(controls.map((control) => control.id)).toEqual(['model-picker', 'auto', 'opaque-mode'])
    config.options = config.options.slice(1)
    expect(configurationControls(config).map((control) => control.id)).toEqual(['auto', 'opaque-mode'])
    config.options = []
    expect(configurationControls(config)).toEqual([])
  })

  it('sends opaque values and real booleans without changing the last confirmed projection', () => {
    const config = configuration()
    const controls = configurationControls(config)
    expect(changeForControl(controls[0], 'provider/model-b')).toEqual({ config_id: 'model-picker', value: { type: 'select', value: 'provider/model-b' } })
    expect(changeForControl(controls[1], true)).toEqual({ config_id: 'auto', value: { type: 'boolean', value: true } })
    expect(changeForControl(controls[2], 'edit')).toEqual({ config_id: 'opaque-mode', value: { type: 'select', value: 'edit' } })
    expect(controls[0].value).toBe('provider/model-a')
    expect(config.options[0].kind.current_value).toBe('provider/model-a')
    expect(changeForControl(controls[0], 'B')).toBeNull()
    expect(changeForControl(controls[0], 'provider/model-a')).toBeNull()
    expect(changeForControl(controls[1], 'true')).toBeNull()
  })

  it('retains server groups and descriptions rather than inventing provider capabilities', () => {
    const control = configurationControls(configuration())[0]
    if (control.type !== 'select') throw new Error('Expected select')
    expect(choiceGroups(control.choices)).toEqual([{ name: 'Provider', choices: control.choices }])
    const { body } = render(SessionConfigurationControls, { props: { configuration: configuration(), onChange: vi.fn(), disabled: true } })
    expect(body).toContain('label="Provider"')
    expect(body).toContain('title="Second model"')
    expect(body).toMatch(/<select[^>]*disabled[^>]*aria-label="Agent model"/)
    expect(body).toContain('aria-pressed="false"')
    expect(body).toContain('aria-label="Mode"')
  })

  it('picks the model as the compact entry and falls back to the first select', () => {
    const controls = configurationControls(configuration())
    expect(primaryConfigurationControl(controls)?.id).toBe('model-picker')
    expect(primaryConfigurationControl(controls.slice(1))?.id).toBe('opaque-mode')
    expect(primaryConfigurationControl([])).toBeNull()
  })

  it('shows the confirmed choice name, falling back to the raw value', () => {
    const controls = configurationControls(configuration())
    expect(controlDisplayValue(controls[0])).toBe('A')
    expect(controlDisplayValue(controls[1])).toBe('Auto approve')

    const config = configuration()
    config.options[0].kind.current_value = 'retired-model'
    expect(controlDisplayValue(configurationControls(config)[0])).toBe('retired-model')
  })

  it('shows an unknown confirmed value without silently picking a different available choice', () => {
    const config = configuration()
    config.options[0].kind.current_value = 'retired-model'
    const { body } = render(SessionConfigurationControls, { props: { configuration: config, onChange: vi.fn() } })
    expect(body).toMatch(/<option[^>]*value="retired-model"[^>]*disabled[^>]*selected/)
  })
})

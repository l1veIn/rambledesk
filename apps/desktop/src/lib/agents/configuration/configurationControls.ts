import type { SessionConfigChange, SessionConfigChoice, SessionConfiguration } from '$lib/generated/feedback'

type ControlBase = Readonly<{ id: string; name: string; description: string | null; source: 'option' | 'mode' | 'model'; configId: string }>
export type ConfigurationControl = ControlBase & (
  { type: 'select'; value: string; choices: readonly SessionConfigChoice[] }
  | { type: 'boolean'; value: boolean }
)

/** Modern Agent options take precedence over their legacy mode/model catalogs. */
export function configurationControls(configuration: SessionConfiguration): ConfigurationControl[] {
  const controls: ConfigurationControl[] = configuration.options.map((option) => ({
    id: `option:${option.id}`, configId: option.id, name: option.name, description: option.description, source: 'option',
    ...(option.kind.type === 'select'
      ? { type: 'select', value: option.kind.current_value, choices: option.kind.options }
      : { type: 'boolean', value: option.kind.current_value }),
  }))
  const categories = new Set(configuration.options.map((option) => option.category?.trim().toLowerCase()))
  if (configuration.modes && !categories.has('mode')) {
    controls.push({ id: 'legacy:mode', configId: '', source: 'mode', name: 'Mode', description: null, type: 'select',
      value: configuration.modes.current_mode_id,
      choices: configuration.modes.available_modes.map((mode) => ({ value: mode.id, name: mode.name, description: mode.description, group: null })),
    })
  }
  if (configuration.models && !categories.has('model')) {
    controls.push({ id: 'legacy:model', configId: '', source: 'model', name: 'Model', description: null, type: 'select',
      value: configuration.models.current_model_id,
      choices: configuration.models.available_models.map((model) => ({ value: model.model_id, name: model.name, description: model.description, group: null })),
    })
  }
  return controls
}

export function changeForControl(control: ConfigurationControl, next: string | boolean): SessionConfigChange | null {
  if (control.value === next || typeof control.value !== typeof next) return null
  if (control.type === 'boolean') return { type: 'option', config_id: control.configId, value: { type: 'boolean', value: next as boolean } }
  if (!control.choices.some((choice) => choice.value === next)) return null
  if (control.source === 'mode') return { type: 'mode', mode_id: next as string }
  if (control.source === 'model') return { type: 'model', model_id: next as string }
  return { type: 'option', config_id: control.configId, value: { type: 'select', value: next as string } }
}

export function choiceGroups(choices: readonly SessionConfigChoice[]) {
  const groups = new Map<string | null, SessionConfigChoice[]>()
  for (const choice of choices) {
    const group = choice.group || null
    const members = groups.get(group) ?? []
    members.push(choice)
    groups.set(group, members)
  }
  return [...groups].map(([name, choices]) => ({ name, choices }))
}

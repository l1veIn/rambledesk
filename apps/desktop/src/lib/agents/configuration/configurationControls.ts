import type { SessionConfigChange, SessionConfigChoice, SessionConfiguration } from '$lib/generated/feedback'

type ControlBase = Readonly<{
  id: string
  name: string
  description: string | null
  /** Agent-provided grouping such as `model` or `mode`; used to pick the compact entry. */
  category: string | null
}>
export type ConfigurationControl = ControlBase & (
  { type: 'select'; value: string; choices: readonly SessionConfigChoice[] }
  | { type: 'boolean'; value: boolean }
)

/** Render application-owned options; protocol routing stays in the driver. */
export function configurationControls(configuration: SessionConfiguration): ConfigurationControl[] {
  return configuration.options.map((option) => ({
    id: option.id, name: option.name, description: option.description, category: option.category,
    ...(option.kind.type === 'select'
      ? { type: 'select', value: option.kind.current_value, choices: option.kind.options }
      : { type: 'boolean', value: option.kind.current_value }),
  }))
}

export function changeForControl(control: ConfigurationControl, next: string | boolean): SessionConfigChange | null {
  if (control.value === next || typeof control.value !== typeof next) return null
  if (control.type === 'boolean') return { config_id: control.id, value: { type: 'boolean', value: next as boolean } }
  if (!control.choices.some((choice) => choice.value === next)) return null
  return { config_id: control.id, value: { type: 'select', value: next as string } }
}

/** The control a compact composer shows as its single entry: the model, else the first select. */
export function primaryConfigurationControl(
  controls: readonly ConfigurationControl[],
): ConfigurationControl | null {
  return (
    controls.find((control) => control.type === 'select' && control.category === 'model') ??
    controls.find((control) => control.type === 'select') ??
    controls[0] ??
    null
  )
}

/** The human-readable current value of a control. */
export function controlDisplayValue(control: ConfigurationControl): string {
  if (control.type === 'boolean') return control.name
  return control.choices.find((choice) => choice.value === control.value)?.name ?? control.value
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

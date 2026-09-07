import type { SessionConfigChange, SessionConfigChoice, SessionConfiguration } from '$lib/generated/feedback'

type ControlBase = Readonly<{ id: string; name: string; description: string | null }>
export type ConfigurationControl = ControlBase & (
  { type: 'select'; value: string; choices: readonly SessionConfigChoice[] }
  | { type: 'boolean'; value: boolean }
)

/** Render application-owned options; protocol routing stays in the driver. */
export function configurationControls(configuration: SessionConfiguration): ConfigurationControl[] {
  return configuration.options.map((option) => ({
    id: option.id, name: option.name, description: option.description,
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

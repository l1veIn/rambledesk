import type { SettingsSection } from '$lib/workbench/types'

export type SettingsSectionCommandState = Readonly<{
  activeSection: SettingsSection
  appliedEpoch: number
}>

export function applySettingsSectionCommand(
  state: SettingsSectionCommandState,
  section: SettingsSection,
  epoch: number,
): SettingsSectionCommandState {
  if (epoch === state.appliedEpoch) return state
  return { activeSection: section, appliedEpoch: epoch }
}

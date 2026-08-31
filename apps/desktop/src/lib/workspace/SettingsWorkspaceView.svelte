<script lang="ts">
  import SettingsPanel from '$lib/SettingsPanel.svelte'
  import type { SettingsSection } from '$lib/workbench/types'

  export let mcpConfiguration = ''
  export let section: SettingsSection = 'general'
  export let sectionSelectionEpoch = 0
  export let updateInstallBlocked = false
  export let onClose: () => void = () => {}
  export let onRestartOnboarding: () => void = () => {}
  export let onOpenArchived: () => void = () => {}
  export let onOpenRambelleProfile: () => void = () => {}
</script>

<SettingsPanel
  presentation="workspace"
  initialSection={section}
  {sectionSelectionEpoch}
  {mcpConfiguration}
  {updateInstallBlocked}
  {onClose}
  {onRestartOnboarding}
  {onOpenArchived}
  {onOpenRambelleProfile}
/>

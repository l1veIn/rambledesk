<script lang="ts">
  import SettingsPanel from '$lib/SettingsPanel.svelte'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import type { SettingsSection } from '$lib/workbench/types'

  export let mcpConfiguration = ''
  export let capabilities: WorkbenchCapabilities
  export let section: SettingsSection = 'general'
  export let sectionSelectionEpoch = 0
  export let updateInstallBlocked = false
  export let onRestartOnboarding: () => void = () => {}
  export let onOpenArchived: () => void = () => {}
  export let onOpenRambelleProfile: () => void = () => {}
</script>

<SettingsPanel
  {capabilities}
  initialSection={section}
  {sectionSelectionEpoch}
  {mcpConfiguration}
  {updateInstallBlocked}
  {onRestartOnboarding}
  {onOpenArchived}
  {onOpenRambelleProfile}
/>

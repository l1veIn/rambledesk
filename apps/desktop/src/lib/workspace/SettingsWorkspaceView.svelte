<script lang="ts">
  import SettingsPanel from '$lib/SettingsPanel.svelte'
  import type { SettingsSection } from '$lib/workbench/types'

  export let mcpConfiguration = ''
  export let section: SettingsSection = 'general'
  export let updateInstallBlocked = false
  export let onClose: () => void = () => {}
  export let onRestartOnboarding: () => void = () => {}
  export let onOpenArchived: () => void = () => {}
</script>

<SettingsPanel
  presentation="workspace"
  initialSection={section}
  {mcpConfiguration}
  {updateInstallBlocked}
  {onClose}
  {onRestartOnboarding}
  {onOpenArchived}
/>

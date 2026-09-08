<script lang="ts">
  import SettingsPanel from '$lib/SettingsPanel.svelte'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import type { SettingsSection } from '$lib/workbench/types'

  export let mcpConfiguration = ''
  export let capabilities: WorkbenchCapabilities
  export let transport: ApplicationTransport
  export let section: SettingsSection = 'general'
  export let sectionSelectionEpoch = 0
  export let agentConfigId: string | undefined = undefined
  export let agentAdvanced = false
  export let updateInstallBlocked = false
  export let onRestartOnboarding: () => void = () => {}
  export let onOpenArchived: () => void = () => {}
  export let onOpenRambelleProfile: () => void = () => {}
</script>

<SettingsPanel
  {transport}
  {capabilities}
  initialSection={section}
  {sectionSelectionEpoch}
  initialAgentConfigId={agentConfigId}
  initialAgentAdvanced={agentAdvanced}
  {mcpConfiguration}
  {updateInstallBlocked}
  {onRestartOnboarding}
  {onOpenArchived}
  {onOpenRambelleProfile}
/>

<!-- Test-only wrapper: `ChangedFilesCard` reads capabilities from context, which
     a directly mounted component cannot provide. Mirrors
     `workspaceTabStripHarness.svelte` in the workspace domain. -->
<script lang="ts">
  import { provideWorkbenchCapabilities } from '$lib/capabilities/capabilityContext'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import ChangedFilesCard from './ChangedFilesCard.svelte'
  import type { ChangedFile } from './changed-files'

  export let capabilities: WorkbenchCapabilities
  export let files: readonly ChangedFile[]
  export let cwd = ''
  export let onOpenDiff: (file: ChangedFile) => void = () => {}

  provideWorkbenchCapabilities(capabilities)
</script>

<ChangedFilesCard {files} {cwd} {onOpenDiff} />

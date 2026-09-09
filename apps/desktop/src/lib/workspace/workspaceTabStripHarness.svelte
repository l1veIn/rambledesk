<script lang="ts">
  // Test harness: lets a test replace the view list and active tab after mount,
  // which is how a new tab actually arrives in the workbench.
  import WorkspaceTabStrip from './WorkspaceTabStrip.svelte'
  import { inboxViewDescriptor, type WorkspaceViewDescriptor } from './viewDescriptors'

  export let views: WorkspaceViewDescriptor[] = [inboxViewDescriptor()]
  export let activeViewKey: string | null = 'inbox:singleton'

  export function setViews(next: WorkspaceViewDescriptor[]) {
    views = next
  }

  export function setActive(viewKey: string) {
    activeViewKey = viewKey
  }
</script>

<WorkspaceTabStrip {views} {activeViewKey} labelForView={(view) => view.kind} />

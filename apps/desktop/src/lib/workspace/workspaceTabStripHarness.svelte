<script lang="ts">
  // Test harness: lets a test replace the view list and active tab after mount,
  // which is how a new tab actually arrives in the workbench.
  import WorkspaceTabStrip from './WorkspaceTabStrip.svelte'
  import { inboxViewDescriptor, workspaceViewKey, type WorkspaceViewDescriptor } from './viewDescriptors'

  export let views: WorkspaceViewDescriptor[] = [inboxViewDescriptor()]
  export let activeViewKey: string | null = 'inbox:singleton'
  export let pendingViewKey: string | null = null
  export let disabled = false
  export let onActivate: (key: string) => void = () => {}
  export let onClose: (key: string) => void = () => {}

  export function setViews(next: WorkspaceViewDescriptor[]) {
    views = next
  }

  export function setActive(viewKey: string) {
    activeViewKey = viewKey
  }

  function activate(key: string) {
    activeViewKey = key
    onActivate(key)
  }

  function close(key: string) {
    views = views.filter((view) => workspaceViewKey(view) !== key)
    if (activeViewKey === key) activeViewKey = views[0] ? workspaceViewKey(views[0]) : null
    onClose(key)
  }
</script>

<WorkspaceTabStrip {views} {activeViewKey} {pendingViewKey} {disabled} labelForView={(view) => view.kind} onActivate={activate} onClose={close} />

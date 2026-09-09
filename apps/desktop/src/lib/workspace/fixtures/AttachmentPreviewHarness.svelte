<script lang="ts">
  import type { ApplicationTransport } from '../../application/applicationTransport'
  import type { WorkbenchCapabilities } from '../../capabilities/workbenchCapabilities'
  import type { AttachmentView } from '../../feedback'
  import RequestAttachmentPreview from '../RequestAttachmentPreview.svelte'

  export let transport: ApplicationTransport
  export let capabilities: Pick<WorkbenchCapabilities, 'serverPaths'>
  let open = false
  let requestId = ''
  let attachment: AttachmentView | null = null
  let readKind: 'request' | 'workspace' = 'request'

  export function show(id: string, item: AttachmentView, kind: 'request' | 'workspace' = 'request') {
    requestId = id
    attachment = item
    readKind = kind
    open = true
  }

  export function close() { open = false }
</script>

<RequestAttachmentPreview {transport} {capabilities} bind:open {requestId} {attachment} {readKind} />

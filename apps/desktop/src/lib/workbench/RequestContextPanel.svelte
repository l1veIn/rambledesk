<script lang="ts">
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import type { FeedbackWorkspaceView } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import MarkdownPreview from '$lib/editor/MarkdownPreview.svelte'
  import RequestAttachments from './RequestAttachments.svelte'

  export let workspace: FeedbackWorkspaceView
  export let transport: ApplicationTransport
  export let capabilities: Pick<WorkbenchCapabilities, 'serverPaths'>
</script>

<section
  class="min-h-0 max-h-[30%] shrink-0 overflow-y-auto overscroll-contain border-b bg-muted/20 px-5 py-3"
  data-request-context
  data-tour="request-context"
  aria-label={t($locale, 'Request context')}
>
  <h2 class="m-0 text-xs font-semibold text-muted-foreground">{t($locale, 'Request context')}</h2>
  <div class="mt-1 text-sm leading-6"><MarkdownPreview markdown={workspace.request.what_happened} bare /></div>
  <RequestAttachments {workspace} {transport} {capabilities} />
</section>

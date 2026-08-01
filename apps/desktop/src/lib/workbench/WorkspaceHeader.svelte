<script lang="ts">
  import type { FeedbackWorkspaceView } from '../feedback'
  import { requestStatusLabel } from '../feedback'
  import { t } from '../i18n'
  import { locale } from '../preferences'
  import type { AdapterPresentation } from './types'

  export let workspace: FeedbackWorkspaceView
  export let adapterPresentation: (hostId: string) => AdapterPresentation
  export let onReload: () => void = () => {}

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }
</script>

<header class="workspace-heading">
  <div class="workspace-heading-copy">
    <div class="workspace-meta">
      <span>{workspace.request.project_name}</span>
      <span class="adapter-chip">
        <i class="adapter-mark" aria-hidden="true">{@html adapterPresentation(workspace.request.agent).icon_svg}</i>
        {adapterPresentation(workspace.request.agent).label}
      </span>
      <span class="status-chip">{requestStatusLabel(workspace.request.status, $locale)}</span>
    </div>
    {#if workspace.request.title.trim()}<h2>{workspace.request.title}</h2>{/if}
    <p>Session · {workspace.request.session_id}</p>
  </div>
  <button class="secondary-button compact-button" onclick={onReload}>{tr('重新载入')}</button>
</header>

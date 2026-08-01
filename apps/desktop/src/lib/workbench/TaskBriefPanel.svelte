<script lang="ts">
  import { ChevronDown } from '@lucide/svelte'

  import type { FeedbackWorkspaceView } from '../feedback'
  import { t } from '../i18n'
  import { locale } from '../preferences'

  export let workspace: FeedbackWorkspaceView
  export let open = true

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }
</script>

<section class:open class="task-sheet">
  <button
    class="task-sheet-toggle"
    aria-expanded={open}
    aria-label={open ? tr('收起') : tr('展开')}
    title={open ? tr('收起') : tr('展开')}
    onclick={() => (open = !open)}
  >
    <span>
      <i>01</i>
      <strong>{tr('任务简报')}</strong>
      <em>{tr('{count} 个体验步骤', { count: workspace.actions.length })}</em>
    </span>
    <b class:open class="task-sheet-toggle-icon">
      <ChevronDown size={20} strokeWidth={1.9} />
    </b>
  </button>

  {#if open}
    <div class="task-sheet-body">
      <section class="brief-section brief-summary-section">
        <p class="eyebrow">WHAT HAPPENED</p>
        <p class="brief-summary">{workspace.request.what_happened}</p>
      </section>

      <section class="brief-section brief-actions-section">
        <p class="eyebrow">WHAT TO TRY</p>
        <ol class="actions">
          {#each workspace.actions as action}
            <li>
              <span>{action.id}</span>
              <p>{action.instruction}</p>
            </li>
          {/each}
        </ol>
      </section>
    </div>
  {/if}
</section>

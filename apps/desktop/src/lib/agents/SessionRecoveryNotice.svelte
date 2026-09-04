<script lang="ts">
  import type { SessionRecovery } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'
  import { agentText } from './agentI18n'
  import { redactAgentMessage } from './agentConfigForm'
  import type { ManagedSessionViewSnapshot } from './managedSessionUi'

  export let snapshot: ManagedSessionViewSnapshot
  export let recovery: SessionRecovery | null = null
  export let envText = ''

  // A checkpoint describes the previous run, never the live process connection.
  $: current = recovery?.session_id === snapshot.session.session_id ? recovery : null
  $: offline = snapshot.runtime.connection !== 'connected' && snapshot.runtime.connection !== 'connecting'
  $: visible = !snapshot.deleting && offline && (current?.status === 'interrupted' || current?.status === 'unclosed')
  $: hasRemoteSession = snapshot.session.management.kind === 'managed' && !!snapshot.session.management.remote_session_id
  $: message = current?.status === 'unclosed'
    ? 'The previous agent run did not finish closing. Its history is preserved.'
    : current?.interrupted_turn_id
      ? 'The previous agent turn was interrupted. Its history is preserved; the turn will not be sent again automatically.'
      : 'The previous agent run was interrupted. Its history is preserved.'
  $: recoveryError = redactAgentMessage(current?.last_error ?? '', envText)
</script>

{#if visible}
  <div role="status" class="shrink-0 space-y-1 border-b border-amber-500/25 bg-amber-500/5 px-5 py-3 text-xs leading-5">
    <p class="m-0">{agentText($locale, message)}</p>
    <p class="m-0 text-muted-foreground">{agentText($locale, hasRemoteSession
      ? 'Choose Resume session in the Agent panel to reconnect to the original agent session.'
      : 'No agent session was established. Choose Start agent in the Agent panel when you are ready.')}</p>
    {#if recoveryError}<p class="m-0 break-words text-muted-foreground">{recoveryError}</p>{/if}
  </div>
{/if}

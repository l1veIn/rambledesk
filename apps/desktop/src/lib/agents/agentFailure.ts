import type { AgentFailure, AgentFailureStage } from '$lib/generated/feedback'
import { redactAgentMessage } from './agentConfigForm'

const stages = new Set(['launch', 'initialize', 'session', 'configuration', 'prompt'])
const reasons = new Set(['authentication', 'configuration', 'model', 'rate_limit', 'network', 'connection', 'unknown'])

/** Only structured evidence can identify authentication or a provider failure. */
export function agentFailureFrom(cause: unknown, fallbackStage: AgentFailureStage, envText = ''): AgentFailure {
  const record = typeof cause === 'object' && cause !== null ? cause as Record<string, unknown> : {}
  const failure = typeof record.failure === 'object' && record.failure !== null ? record.failure as Record<string, unknown> : record
  const known = typeof failure.stage === 'string' && stages.has(failure.stage)
    && typeof failure.reason === 'string' && reasons.has(failure.reason)
  const source = known && typeof failure.message === 'string' ? failure.message
    : typeof record.message === 'string' ? record.message : 'The agent operation did not complete.'
  return {
    stage: known ? failure.stage as AgentFailure['stage'] : fallbackStage,
    reason: known ? failure.reason as AgentFailure['reason'] : 'unknown',
    message: redactAgentMessage(source, envText),
  }
}

export function agentFailureTitle(failure: AgentFailure): string {
  return {
    launch: 'Could not start the ACP program',
    initialize: 'Could not connect ACP',
    session: 'Could not prepare this session',
    configuration: 'Could not change the session option',
    prompt: 'The agent could not complete this message',
  }[failure.stage]
}

export function agentFailureAdvice(failure: AgentFailure): string {
  switch (failure.reason) {
    case 'authentication': return 'This connection requires sign-in or an API key. Complete authentication in the environment used by this agent.'
    case 'model': return 'The selected model is unavailable. Choose another model or review model access in the agent configuration.'
    case 'configuration': return 'The agent rejected its configuration. Review the selected session options or this connection’s advanced settings.'
    case 'rate_limit': return 'The model provider is limiting requests. Wait before retrying, or select another available model.'
    case 'network': return 'The agent could not reach its service. Check the network used by the agent, then retry.'
    case 'connection': return 'The ACP connection ended during this action. Reconnect this session before continuing.'
    default: return failure.stage === 'launch' || failure.stage === 'initialize'
      ? 'Check the ACP program location, connection component, and launch diagnostics in agent settings.'
      : 'Review the details for this action, then retry. Your project directory and unsent text are preserved.'
  }
}

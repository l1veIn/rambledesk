<script lang="ts">
  import { Copy, Check, ExternalLink } from '@lucide/svelte'
  import type { AgentConfig, AgentInspection } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'
  import { agentSetupGuidance } from './agentOnboarding'

  export let catalogId: string | undefined = undefined
  export let hostId: string | undefined = undefined
  export let name: string | undefined = undefined
  export let config: AgentConfig | null | undefined = undefined
  export let inspection: AgentInspection | undefined = undefined
  export let compact = false
  export let purpose: 'authentication' | 'troubleshooting' = 'troubleshooting'
  export let onConfigure: (() => void) | undefined = undefined
  let copied = ''
  let copyError = ''
  $: guide = agentSetupGuidance({ catalogId, hostId, name, config, inspection,
    platform: typeof navigator !== 'undefined' && /Win/iu.test(navigator.platform) ? 'windows' : undefined })
  function tr(zh: string, en: string) { return $locale === 'zh-CN' ? zh : en }
  async function copy() {
    if (!guide.command) return
    const command = guide.command
    copyError = ''
    try { await navigator.clipboard.writeText(command); copied = command }
    catch { copyError = tr('复制失败，请选择并复制上面的命令。', 'Could not copy. Select and copy the command above.') }
  }
</script>

<section class:rounded-lg={!compact} class:border={!compact} class:p-3={!compact} class="space-y-2 text-xs leading-5" aria-label={tr(`${guide.name} 配置方法`, `${guide.name} setup guide`)} data-agent-setup-guide>
  {#if !compact}<h5 class="m-0 text-xs font-medium">{purpose === 'authentication' ? tr(`为 ${guide.name} 完成认证`, `Authenticate ${guide.name}`) : tr('检查 ACP 连接设置', 'Check ACP connection settings')}</h5>{/if}
  {#if !compact || !guide.note}<p class="m-0 text-muted-foreground">{purpose === 'authentication'
    ? tr('当前连接使用的智能体要求认证。请在同一台机器、同一用户环境中登录，或按智能体说明配置 API key，然后重试当前会话。', 'The agent used by this connection requires authentication. Sign in on the same machine and user account, or configure an API key as instructed by the agent, then retry this session.')
    : tr('请在智能体设置中检查 ACP 启动入口、连接组件和诊断结果，再重试当前操作。', 'Check the ACP launch entry, connection component, and diagnostics in agent settings, then retry the current action.')}</p>{/if}
  {#if purpose === 'authentication' && guide.command}
    <p class="m-0 text-[11px] text-muted-foreground">{guide.platform === 'windows' ? tr('在 PowerShell 中运行：', 'Run in PowerShell:') : tr('在终端中运行：', 'Run in a terminal:')}</p>
    <div class="flex items-start gap-2 rounded-md bg-muted/60 px-3 py-2">
      <code class="min-w-0 flex-1 select-all whitespace-pre-wrap break-all font-mono text-[11px]">{guide.command}</code>
      <button type="button" class="shrink-0 rounded p-1 hover:bg-muted" aria-label={tr(copied === guide.command ? '已复制' : '复制命令', copied === guide.command ? 'Copied' : 'Copy command')} onclick={copy}>{#if copied === guide.command}<Check class="size-3.5" />{:else}<Copy class="size-3.5" />{/if}</button>
    </div>
    {#if copyError}<p role="status" class="m-0 text-muted-foreground">{copyError}</p>{/if}
  {/if}
  {#if purpose === 'authentication' && guide.note}<p class="m-0 text-muted-foreground">{tr(...guide.note)}</p>{/if}
  {#if onConfigure}<button type="button" class="inline-block text-left underline underline-offset-4" onclick={onConfigure}>{purpose === 'authentication' ? tr('在高级设置中配置 API key', 'Configure an API key in advanced settings') : tr('打开智能体设置', 'Open agent settings')}</button>{/if}
  {#if guide.guide}<a href={guide.guide} target="_blank" rel="noreferrer" class="inline-flex items-center gap-1 underline underline-offset-4">{tr('安装与配置说明', 'Installation and setup guide')}<ExternalLink class="size-3" /></a>{/if}
  {#if guide.hasLaunchOverrides}<p class="m-0 text-[11px] text-muted-foreground">{tr('已有的启动覆盖设置仍然保留。若智能体自身的设置未生效，可在高级启动设置中检查。', 'Existing launch overrides are still saved. If the agent’s own settings do not take effect, check the advanced launch settings.')}</p>{/if}
</section>

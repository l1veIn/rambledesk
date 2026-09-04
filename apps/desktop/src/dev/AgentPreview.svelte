<script lang="ts">
  import AgentCatalog from '$lib/agents/AgentCatalog.svelte'
  import ManagedSessionSection from '$lib/agents/ManagedSessionSection.svelte'
  import { setLocale } from '$lib/preferences'
  import { transport } from './agentPreviewFixtures'
  let page = 'agents'
  setLocale('zh-CN')
  function toggleTheme() {
    const next = document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark'
    document.documentElement.dataset.theme = next
    document.documentElement.style.colorScheme = next
  }
</script>
<div class="flex h-screen flex-col bg-background text-foreground">
  <nav class="flex shrink-0 items-center gap-4 border-b px-6 py-3 text-sm"><strong>RambleDesk</strong><span class="text-xs text-muted-foreground">隔离界面预览</span><button onclick={() => page = 'agents'}>智能体管理</button><button onclick={() => page = 'chat'}>项目会话</button><button class="ml-auto" onclick={toggleTheme}>明暗主题</button></nav>
  {#if page === 'agents'}<main class="mx-auto w-full max-w-5xl flex-1 overflow-auto p-6"><AgentCatalog {transport} /></main>
  {:else}<main class="mx-auto flex min-h-0 w-full max-w-5xl flex-1 flex-col border-x"><ManagedSessionSection {transport} sessionId="preview" onOpenFeedback={() => {}} /></main>{/if}
</div>

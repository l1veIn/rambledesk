<script lang="ts">
  // Interactive product mock for the marketing page. It follows the real workbench layout:
  // titlebar with workspace tabs, project/session rail, request rail, then the workspace pane
  // (task brief above the feedback document) and the command rail. Every object below is
  // scripted demonstration data from content/product-mock.ts — no transport, no agent, no network.
  import { onDestroy } from 'svelte'

  import MockCommandRail from './MockCommandRail.svelte'
  import MockWorkspace from './MockWorkspace.svelte'
  import {
    mockContent,
    type DeliveryState,
    type DocumentBlock,
    type MockProject,
    type MockRequest,
    type MockSession,
    type RequestStatus,
  } from '../content/product-mock'

  let { lang = 'en' }: { lang: string } = $props()

  interface MockAttachment {
    name: string
    mediaType: string
    sizeKiB: number
  }

  interface RequestOverride {
    status?: RequestStatus
    delivery?: DeliveryState
  }

  const instanceId = $props.id()
  const content = $derived(mockContent(lang))
  const ui = $derived(content.ui)

  type ViewKind = 'request' | 'agent' | 'settings'

  let sessionChoice = $state<string | null>(null)
  let requestChoice = $state<string | null>(null)
  let viewKind = $state<ViewKind>('request')
  let collapsedProjects = $state<string[]>([])
  let sidebarCollapsed = $state(false)
  let searchOpen = $state(false)
  let searchQuery = $state('')
  let inboxScope = $state(false)
  let filterOpen = $state(false)
  let hiddenStatuses = $state<RequestStatus[]>([])
  let documents = $state<Record<string, DocumentBlock[]>>({})
  let attachments = $state<Record<string, MockAttachment[]>>({})
  let overrides = $state<Record<string, RequestOverride>>({})
  let packageOpen = $state<Record<string, boolean>>({})
  let versions = $state<Record<string, 'cooked' | 'uncooked'>>({})
  let activeActions = $state<Record<string, string | null>>({})
  let briefOpen = $state(true)
  let editingBlockId = $state<string | null>(null)
  let previewCapture = $state<string | null>(null)
  let submitting = $state(false)
  let ramblePhase = $state<'idle' | 'recording'>('idle')
  let tidyBusy = $state(false)
  let savePhase = $state<'saved' | 'saving' | 'unsaved'>('saved')
  let savedRevision = $state(1)
  let blockSeq = 0
  let timers: ReturnType<typeof setTimeout>[] = []

  const allSessions = $derived(content.sessions)
  const session = $derived(
    allSessions.find((item) => item.id === sessionChoice) ?? allSessions[0] ?? null,
  )
  const project = $derived(
    content.projects.find((item) => session && item.sessionIds.includes(session.id)) ?? null,
  )
  const sessionRequests = $derived(
    (session?.requestIds ?? []).map((id) => content.requests[id]).filter(Boolean),
  )
  const visibleRequests = $derived(
    (inboxScope ? Object.values(content.requests) : sessionRequests).filter(
      (item) => !hiddenStatuses.includes(effectiveStatus(item)),
    ),
  )
  const request = $derived(
    requestChoice && content.requests[requestChoice]
      ? content.requests[requestChoice]
      : (visibleRequests[0] ?? sessionRequests[0] ?? null),
  )
  const host = $derived(content.hosts.find((item) => item.id === session?.hostId) ?? content.hosts[0])
  const blocks = $derived(
    (request ? documents[request.id] : null) ?? request?.document ?? [],
  )
  const requestAttachments = $derived((request ? attachments[request.id] : null) ?? [])
  const status = $derived(request ? effectiveStatus(request) : 'waiting')
  const delivery = $derived(
    request ? (overrides[request.id]?.delivery ?? request.delivery) : 'none',
  )
  const version = $derived(request ? (versions[request.id] ?? 'uncooked') : 'uncooked')
  const pendingSpeech = $derived(blocks.filter((block) => block.kind === 'speech' && block.pending).length)
  const segmentCount = $derived(blocks.filter((block) => block.kind === 'speech').length)
  const readOnly = $derived(status === 'completed' || status === 'cancelled')
  const packages = $derived(request?.packageFiles ?? [])
  const scopeLabel = $derived(
    inboxScope ? ui.allRequests : (project?.cwd ?? project?.name ?? ui.scope),
  )
  const filteredProjects = $derived(
    searchQuery.trim()
      ? content.projects
          .map((item) => ({
            ...item,
            sessionIds: item.sessionIds.filter((id) =>
              sessionsFor(item)
                .find((candidate) => candidate.id === id)
                ?.title.toLowerCase()
                .includes(searchQuery.trim().toLowerCase()),
            ),
          }))
          .filter((item) => item.sessionIds.length > 0)
      : content.projects,
  )

  function effectiveStatus(item: MockRequest): RequestStatus {
    return overrides[item.id]?.status ?? item.status
  }

  function sessionsFor(item: MockProject): MockSession[] {
    return item.sessionIds
      .map((id) => allSessions.find((candidate) => candidate.id === id))
      .filter((candidate): candidate is MockSession => Boolean(candidate))
  }

  function prefersReducedMotion() {
    return typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches
  }

  function later(ms: number, run: () => void) {
    const timer = setTimeout(
      () => {
        timers = timers.filter((item) => item !== timer)
        run()
      },
      prefersReducedMotion() ? 0 : ms,
    )
    timers.push(timer)
  }

  function clearTimers() {
    for (const timer of timers) clearTimeout(timer)
    timers = []
  }

  onDestroy(clearTimers)

  /** The first write seeds the draft from the scripted document, then edits stay local. */
  function ensureDocument(): DocumentBlock[] {
    if (!request) return []
    if (!documents[request.id]) {
      documents[request.id] = request.document.map((block) => ({ ...block }))
    }
    return documents[request.id]
  }

  function nextBlockId(prefix: string) {
    blockSeq += 1
    return `${prefix}-${blockSeq}`
  }

  function autosave() {
    clearTimers()
    savePhase = 'unsaved'
    later(420, () => {
      savePhase = 'saving'
      later(520, () => {
        savePhase = 'saved'
        savedRevision += 1
      })
    })
  }

  function resetTransient() {
    clearTimers()
    editingBlockId = null
    previewCapture = null
    ramblePhase = 'idle'
    tidyBusy = false
    submitting = false
    savePhase = 'saved'
  }

  function selectSession(id: string) {
    if (id === session?.id) return
    sessionChoice = id
    inboxScope = false
    requestChoice = null
    const next = allSessions.find((item) => item.id === id)
    viewKind = next && next.requestIds.length > 0 ? 'request' : 'agent'
    resetTransient()
  }

  function selectRequest(id: string) {
    if (id === request?.id && viewKind === 'request') return
    requestChoice = id
    viewKind = 'request'
    resetTransient()
  }

  function toggleProject(key: string) {
    collapsedProjects = collapsedProjects.includes(key)
      ? collapsedProjects.filter((item) => item !== key)
      : [...collapsedProjects, key]
  }

  function selectAction(actionId: string, instruction: string) {
    if (!request || readOnly) return
    const current = ensureDocument()
    current.push({ kind: 'action', id: nextBlockId('action'), label: ui.actionGroupLabel, text: instruction })
    activeActions[request.id] = actionId
    autosave()
  }

  function commitBlock(id: string, text: string) {
    editingBlockId = null
    const current = ensureDocument()
    const block = current.find((item) => item.id === id)
    if (!block || !('text' in block) || block.text === text) return
    block.text = text
    autosave()
  }

  function toggleRecording() {
    if (!request || readOnly) return
    if (ramblePhase === 'idle') {
      ramblePhase = 'recording'
      return
    }
    ramblePhase = 'idle'
    const current = ensureDocument()
    current.push({
      kind: 'speech',
      id: nextBlockId('speech'),
      text: request.speechLine || request.speechTidy,
      pending: true,
    })
    autosave()
  }

  function tidySpeech() {
    if (!request || readOnly || tidyBusy || pendingSpeech === 0) return
    tidyBusy = true
    later(900, () => {
      const current = ensureDocument()
      for (const block of current) {
        if (block.kind === 'speech' && block.pending) {
          block.text = request.speechTidy || block.text
          block.pending = false
        }
      }
      tidyBusy = false
      autosave()
    })
  }

  function addContext(kind: 'capture' | 'clipboard' | 'files') {
    if (!request || readOnly) return
    const preset =
      kind === 'capture'
        ? { name: request.captureName, mediaType: 'image/png', sizeKiB: 312 }
        : kind === 'clipboard'
          ? { name: 'clipboard-paste.png', mediaType: 'image/png', sizeKiB: 148 }
          : { name: 'notes.md', mediaType: 'text/markdown', sizeKiB: 1.8 }
    const list = attachments[request.id] ?? (attachments[request.id] = [])
    if (!list.some((item) => item.name === preset.name)) list.push(preset)
    const current = ensureDocument()
    if (preset.mediaType.startsWith('image/')) {
      current.push({
        kind: 'capture',
        id: nextBlockId('capture'),
        name: preset.name,
        caption: request.captureCaption,
      })
    } else {
      current.push({
        kind: 'text',
        id: nextBlockId('ref'),
        text: `[${preset.name}] ${ui.documentHint}`,
      })
    }
    autosave()
  }

  function removeAttachment(name: string) {
    if (!request) return
    attachments[request.id] = (attachments[request.id] ?? []).filter((item) => item.name !== name)
    const current = ensureDocument()
    documents[request.id] = current.filter(
      (block) => !(block.kind === 'capture' && block.name === name),
    )
    autosave()
  }

  function submitFeedback() {
    if (!request || readOnly || submitting) return
    submitting = true
    later(1100, () => {
      submitting = false
      overrides[request.id] = { ...overrides[request.id], status: 'completed', delivery: 'pending' }
      later(700, () => {
        overrides[request.id] = { ...overrides[request.id], status: 'completed', delivery: 'sending' }
        later(900, () => {
          overrides[request.id] = { ...overrides[request.id], status: 'completed', delivery: 'delivered' }
        })
      })
    })
  }

  function cancelFeedback() {
    if (!request || readOnly) return
    overrides[request.id] = { ...overrides[request.id], status: 'cancelled', delivery: 'none' }
    resetTransient()
  }

  function resetExample() {
    clearTimers()
    sessionChoice = null
    requestChoice = null
    viewKind = 'request'
    collapsedProjects = []
    sidebarCollapsed = false
    searchOpen = false
    searchQuery = ''
    inboxScope = false
    filterOpen = false
    hiddenStatuses = []
    documents = {}
    attachments = {}
    overrides = {}
    packageOpen = {}
    versions = {}
    activeActions = {}
    briefOpen = true
    blockSeq = 0
    savePhase = 'saved'
    savedRevision = 1
    resetTransient()
  }

  function onTabKey(event: KeyboardEvent, index: number) {
    const tabs: ViewKind[] = ['request', 'agent', 'settings']
    let next: number
    if (event.key === 'ArrowRight' || event.key === 'ArrowDown') next = (index + 1) % tabs.length
    else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') next = (index + tabs.length - 1) % tabs.length
    else if (event.key === 'Home') next = 0
    else if (event.key === 'End') next = tabs.length - 1
    else return
    event.preventDefault()
    viewKind = tabs[next]
  }
</script>

<div class="product-mock">
  <div class="mock-strip">
    <span class="strip-label"><i aria-hidden="true"></i>{ui.exampleLabel}</span>
    <button type="button" class="strip-reset" onclick={resetExample}>{ui.reset}</button>
  </div>

  <div class="app-frame">
    <main class="app-window" id={`${instanceId}-window`}>
      <header class="titlebar">
        <div class="titlebar-sidebar" class:collapsed={sidebarCollapsed}>
          <img src="/assets/refresh/rambledesk-app-icon.webp" alt="" width="28" height="28" decoding="async" />
          {#if !sidebarCollapsed}<strong>RambleDesk</strong>{/if}
        </div>
        <div class="tab-strip" role="tablist" aria-label={ui.workspaceTabs ?? 'Workspace'}>
          {#each [{ kind: 'request' as ViewKind, label: request?.title ?? session?.title ?? '' }, { kind: 'agent' as ViewKind, label: ui.agentTab }, ...(viewKind === 'settings' ? [{ kind: 'settings' as ViewKind, label: ui.settingsTab }] : [])] as tab, index (tab.kind)}
            <button
              type="button"
              role="tab"
              class="workspace-tab"
              aria-selected={viewKind === tab.kind}
              tabindex={viewKind === tab.kind ? 0 : -1}
              onclick={() => (viewKind = tab.kind)}
              onkeydown={(event) => onTabKey(event, index)}
            >
              {#if tab.kind === 'settings'}
                <svg class="icon" width="13" height="13" viewBox="0 0 24 24" aria-hidden="true"><path d="m9 3-1 3-3 1v4l2 1-2 1v4l3 1 1 3h6l1-3 3-1v-4l-2-1 2-1V7l-3-1-1-3Z" /><circle cx="12" cy="12" r="3" /></svg>
              {/if}
              <span class="truncate">{tab.label}</span>
            </button>
          {/each}
        </div>
        <div class="window-controls" aria-hidden="true">
          <span></span><span></span><span></span>
        </div>
      </header>

      <div class="mobile-strips">
        <div class="strip-row" role="group" aria-label={ui.projects}>
          {#each allSessions as item (item.id)}
            <button
              type="button"
              class="strip-chip"
              aria-current={item.id === session?.id ? 'true' : undefined}
              onclick={() => selectSession(item.id)}
            >
              {item.title}
            </button>
          {/each}
        </div>
        {#if visibleRequests.length > 1}
          <div class="strip-row" role="group" aria-label={ui.requests}>
            {#each visibleRequests as item (item.id)}
              <button
                type="button"
                class="strip-chip"
                aria-current={item.id === request?.id ? 'true' : undefined}
                onclick={() => selectRequest(item.id)}
              >
                {item.title}
              </button>
            {/each}
          </div>
        {/if}
      </div>

      <div class="app-body" class:rail-collapsed={sidebarCollapsed}>
        <aside class="host-rail" aria-label={ui.projects}>
          <div class="rail-head">
            {#if !sidebarCollapsed}
              <strong class="truncate">RambleDesk</strong>
              <button type="button" class="rail-icon" aria-pressed={searchOpen} aria-label={ui.search} onclick={() => (searchOpen = !searchOpen)}>
                <svg class="icon" width="15" height="15" viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" /></svg>
              </button>
              <button type="button" class="rail-icon" aria-pressed={inboxScope} aria-label={ui.allRequests} onclick={() => { inboxScope = !inboxScope; requestChoice = null }}>
                <svg class="icon" width="15" height="15" viewBox="0 0 24 24" aria-hidden="true"><path d="M3 12h4l2 3h6l2-3h4" /><path d="M5 5h14l2 7v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-5Z" /></svg>
                {#if allSessions.some((item) => item.pendingRequests > 0)}<span class="pending-dot" aria-hidden="true"></span>{/if}
              </button>
            {/if}
            <button type="button" class="rail-icon" aria-label={sidebarCollapsed ? ui.expandSidebar : ui.collapseSidebar} onclick={() => (sidebarCollapsed = !sidebarCollapsed)}>
              <svg class="icon" width="15" height="15" viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="4" width="18" height="16" rx="2" /><path d="M9 4v16" /><path d={sidebarCollapsed ? 'm14 9 3 3-3 3' : 'm17 9-3 3 3 3'} /></svg>
            </button>
          </div>

          <div class="new-session-row">
            <span class="new-session" aria-hidden="true">
              <svg class="icon" width="14" height="14" viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" /></svg>
              {#if !sidebarCollapsed}{ui.newSession}{/if}
            </span>
          </div>

          {#if searchOpen && !sidebarCollapsed}
            <div class="search-row">
              <input
                type="search"
                value={searchQuery}
                aria-label={ui.search}
                placeholder={ui.search}
                oninput={(event) => (searchQuery = event.currentTarget.value)}
              />
            </div>
          {/if}

          {#if !sidebarCollapsed}
            <p class="rail-label">{ui.projects}</p>
          {/if}

          <div class="project-list">
            {#each filteredProjects as item (item.key)}
              <section class="project">
                {#if !sidebarCollapsed}
                  <button type="button" class="project-head" aria-expanded={!collapsedProjects.includes(item.key)} onclick={() => toggleProject(item.key)}>
                    <svg class="icon" width="14" height="14" viewBox="0 0 24 24" aria-hidden="true">
                      <path d={collapsedProjects.includes(item.key) ? 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z' : 'M3 8a2 2 0 0 1 2-2h4l2 2h6a2 2 0 0 1 2 2H3Z'} />
                      {#if !collapsedProjects.includes(item.key)}<path d="M3 10h18v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" />{/if}
                    </svg>
                    <span class="truncate" title={item.cwd ?? item.name}>{item.kind === 'external' ? ui.externalSessions : item.name}</span>
                    <svg class="icon chevron" class:open={!collapsedProjects.includes(item.key)} width="12" height="12" viewBox="0 0 24 24" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg>
                  </button>
                {/if}
                {#if sidebarCollapsed || !collapsedProjects.includes(item.key)}
                  {#each sessionsFor(item) as entry (entry.id)}
                    <button
                      type="button"
                      class="session-row"
                      class:collapsed={sidebarCollapsed}
                      aria-current={entry.id === session?.id ? 'page' : undefined}
                      aria-label={`${entry.title} · ${content.hosts.find((candidate) => candidate.id === entry.hostId)?.label ?? ''}`}
                      title={sidebarCollapsed ? entry.title : undefined}
                      onclick={() => selectSession(entry.id)}
                    >
                      {#if sidebarCollapsed}
                        <span class="host-mark" style={`--tone: ${content.hosts.find((candidate) => candidate.id === entry.hostId)?.tone ?? '#2775ca'}`} aria-hidden="true">
                          <svg width="12" height="13" viewBox="0 0 24 26"><path d="m12 2 10 6v10l-10 6L2 18V8Z" /></svg>
                        </span>
                      {:else}
                        <span class="truncate">{entry.title}</span>
                      {/if}
                      {#if entry.pendingRequests > 0}<span class="pending-dot" aria-hidden="true"></span>{/if}
                    </button>
                  {/each}
                {/if}
              </section>
            {:else}
              <p class="rail-empty">{ui.noSessions ?? ui.noRequests}</p>
            {/each}
          </div>

          <div class="rail-foot">
            <button type="button" class="settings-button" class:active={viewKind === 'settings'} aria-pressed={viewKind === 'settings'} onclick={() => (viewKind = 'settings')}>
              <svg class="icon" width="15" height="15" viewBox="0 0 24 24" aria-hidden="true"><path d="m9 3-1 3-3 1v4l2 1-2 1v4l3 1 1 3h6l1-3 3-1v-4l-2-1 2-1V7l-3-1-1-3Z" /><circle cx="12" cy="12" r="3" /></svg>
              {#if !sidebarCollapsed}{ui.settings}{/if}
            </button>
          </div>
        </aside>

        <aside class="request-rail" aria-label={ui.requestList}>
          <div class="rail-head requests-head">
            <strong>{ui.requests}</strong>
            {#if visibleRequests.length > 0}<span class="count-badge">{visibleRequests.length}</span>{/if}
            <span class="scope truncate" title={scopeLabel}>{scopeLabel}</span>
            <button type="button" class="rail-icon" aria-pressed={filterOpen} aria-label={ui.filter} onclick={() => (filterOpen = !filterOpen)}>
              <svg class="icon" width="15" height="15" viewBox="0 0 24 24" aria-hidden="true"><path d="M3 6h18M7 12h10M10 18h4" /></svg>
            </button>
          </div>

          {#if filterOpen}
            <div class="filter-panel">
              {#each (['waiting', 'in_progress', 'completed', 'cancelled'] as RequestStatus[]) as value (value)}
                <label class="filter-row">
                  <input
                    type="checkbox"
                    checked={!hiddenStatuses.includes(value)}
                    onchange={(event) =>
                      (hiddenStatuses = event.currentTarget.checked
                        ? hiddenStatuses.filter((item) => item !== value)
                        : [...hiddenStatuses, value])}
                  />
                  <span>{ui.requestStatus[value]}</span>
                </label>
              {/each}
            </div>
          {/if}

          {#if visibleRequests.length > 0}
            <nav class="request-list" aria-label={ui.requests}>
              {#each visibleRequests as item (item.id)}
                <button
                  type="button"
                  class="request-card"
                  aria-current={item.id === request?.id ? 'true' : undefined}
                  onclick={() => selectRequest(item.id)}
                >
                  <span class="request-top">
                    <strong class="truncate">{item.title}</strong>
                    <span class="status-badge {effectiveStatus(item)}">{ui.requestStatus[effectiveStatus(item)]}</span>
                  </span>
                  <span class="request-preview">{item.whatHappened}</span>
                  <span class="request-meta">
                    <span class="host-mark" style={`--tone: ${content.hosts.find((candidate) => candidate.id === item.hostId)?.tone ?? '#2775ca'}`} aria-hidden="true">
                      <svg width="10" height="11" viewBox="0 0 24 26"><path d="m12 2 10 6v10l-10 6L2 18V8Z" /></svg>
                    </span>
                    <span class="truncate">{item.sourceHint}</span>
                    <span class="tabular">{item.updatedAt}</span>
                  </span>
                </button>
              {/each}
            </nav>
          {:else}
            <div class="rail-empty-state">
              <span class="empty-icon" aria-hidden="true">
                <svg class="icon" width="16" height="16" viewBox="0 0 24 24"><path d="M3 12h4l2 3h6l2-3h4" /><path d="M5 5h14l2 7v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-5Z" /></svg>
              </span>
              <strong>{ui.noRequests}</strong>
              <span>{ui.noRequestsHint}</span>
            </div>
          {/if}
        </aside>

        <MockWorkspace
          {ui}
          {viewKind}
          {request}
          {session}
          hostLabel={host.label}
          hostTone={host.tone}
          {status}
          {delivery}
          {submitting}
          {briefOpen}
          {blocks}
          attachments={requestAttachments}
          activeActionId={request ? (activeActions[request.id] ?? null) : null}
          {editingBlockId}
          {pendingSpeech}
          {tidyBusy}
          {version}
          {savePhase}
          {savedRevision}
          {previewCapture}
          onToggleBrief={() => (briefOpen = !briefOpen)}
          onSelectAction={selectAction}
          onEditBlock={(id) => (editingBlockId = id)}
          onCommitBlock={commitBlock}
          onTidy={tidySpeech}
          onSetVersion={(next) => {
            if (request) versions[request.id] = next
          }}
          onPreviewAttachment={(name) => (previewCapture = name)}
        >
          {#snippet rail()}
            <MockCommandRail
              {ui}
              {status}
              {delivery}
              attachments={requestAttachments}
              {ramblePhase}
              {segmentCount}
              {submitting}
              packageOpen={request ? Boolean(packageOpen[request.id]) : false}
              packageFiles={packages}
              onToggleRecording={toggleRecording}
              onCapture={() => addContext('capture')}
              onClipboard={() => addContext('clipboard')}
              onFiles={() => addContext('files')}
              onRemoveAttachment={removeAttachment}
              onPreviewAttachment={(name) => (previewCapture = name)}
              onSubmit={submitFeedback}
              onCancel={cancelFeedback}
              onTogglePackage={() => {
                if (request) packageOpen[request.id] = !packageOpen[request.id]
              }}
            />
          {/snippet}
        </MockWorkspace>
      </div>
    </main>
  </div>

  <p class="mock-footnote">{ui.footerNote}</p>
</div>

<style>
  .product-mock {
    --rd-border: #c8d5e3;
    --rd-muted: #edf3f8;
    --rd-muted-foreground: #60738a;
    --rd-primary: #2775ca;
    width: min(100%, var(--feedback-width));
    margin-inline: auto;
    color: #20334b;
    /* The mock collapses on its own width, not the viewport's, so a narrow reading column
       still shows the full three-rail window when the page is wide enough. */
    container-type: inline-size;
  }
  .mock-strip {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 12px;
  }
  .strip-label {
    display: inline-flex;
    align-items: center;
    gap: 9px;
    color: #cfe0ec;
    font-size: 12px;
    font-weight: 600;
  }
  .strip-label i {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #57c6c0;
  }
  .strip-reset {
    min-height: 34px;
    padding: 7px 13px;
    border: 1px solid #6d8ba3;
    border-radius: 4px;
    background: #ffffff12;
    color: #e2eef6;
    font-size: 12px;
  }
  .strip-reset:hover {
    border-color: #9db9cd;
    background: #ffffff22;
  }
  .mock-footnote {
    margin: 12px 0 0;
    color: #a9c0d2;
    font-size: 11px;
    line-height: 1.7;
  }
  .app-frame {
    padding: 1px;
    border-radius: 16px;
    background: #e3e9f0;
    box-shadow: 0 26px 65px #07131f33;
  }
  .app-window {
    display: flex;
    flex-direction: column;
    /* The desktop window is created at its own minimum size: 1320 × 840. The mock keeps that
       aspect ratio, so at full width it is exactly one real window and scales down from there. */
    aspect-ratio: 1320 / 840;
    overflow: hidden;
    border: 1px solid var(--rd-border);
    border-radius: 15px;
    background: #f7f9fc;
  }
  .icon {
    flex: 0 0 auto;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .truncate {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tabular {
    flex: 0 0 auto;
    font-variant-numeric: tabular-nums;
  }

  /* Titlebar */
  .titlebar {
    display: flex;
    align-items: stretch;
    flex-shrink: 0;
    height: 40px;
    background: #dfe7f0;
    border-bottom: 1px solid var(--rd-border);
  }
  .titlebar-sidebar {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 240px;
    flex: 0 0 auto;
    padding: 0 12px;
    border-right: 1px solid var(--rd-border);
    background: #f7f9fc;
    transition: width 200ms ease;
  }
  .titlebar-sidebar.collapsed {
    width: 56px;
    justify-content: center;
    padding: 0 6px;
  }
  .titlebar-sidebar img {
    width: 28px;
    height: 28px;
    border-radius: 7px;
  }
  .titlebar-sidebar strong {
    font-size: 12px;
    font-weight: 600;
  }
  .tab-strip {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    min-width: 0;
    flex: 1;
    padding: 4px 6px 0;
    overflow: hidden;
  }
  .workspace-tab {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    max-width: 230px;
    height: 32px;
    padding: 0 12px;
    border: 0;
    border-radius: 7px 7px 0 0;
    background: none;
    color: #4b6277;
    font-size: 11px;
  }
  .workspace-tab:hover {
    background: rgb(255 255 255 / 55%);
  }
  .workspace-tab[aria-selected='true'] {
    background: #f7f9fc;
    color: #20334b;
    font-weight: 550;
    box-shadow: 0 -1px 0 var(--rd-border), 1px 0 0 var(--rd-border), -1px 0 0 var(--rd-border);
  }
  .window-controls {
    display: flex;
    align-items: stretch;
    flex: 0 0 auto;
  }
  .window-controls span {
    position: relative;
    width: 44px;
  }
  .window-controls span::before,
  .window-controls span::after {
    position: absolute;
    top: 50%;
    left: 50%;
    background: #7b8fa1;
    content: '';
    transform: translate(-50%, -50%);
  }
  .window-controls span:nth-child(1)::before {
    width: 11px;
    height: 1px;
  }
  .window-controls span:nth-child(2)::before {
    width: 9px;
    height: 9px;
    border: 1px solid #7b8fa1;
    background: none;
  }
  .window-controls span:nth-child(3)::before {
    width: 11px;
    height: 1px;
    transform: translate(-50%, -50%) rotate(45deg);
  }
  .window-controls span:nth-child(3)::after {
    width: 11px;
    height: 1px;
    transform: translate(-50%, -50%) rotate(-45deg);
  }

  /* Body columns */
  .app-body {
    display: grid;
    grid-template-columns: 240px 240px minmax(0, 1fr);
    flex: 1 1 auto;
    min-height: 0;
    overflow: hidden;
  }
  .app-body.rail-collapsed {
    grid-template-columns: 56px 240px minmax(0, 1fr);
  }

  /* Host rail */
  .host-rail {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    border-right: 1px solid var(--rd-border);
    background: #f7f9fc;
  }
  .rail-head {
    display: flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
    min-width: 0;
    height: 48px;
    padding: 0 10px;
  }
  .rail-head strong {
    min-width: 0;
    flex: 1;
    font-size: 12px;
    font-weight: 600;
  }
  .rail-icon {
    position: relative;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    flex: 0 0 auto;
    border: 0;
    border-radius: 6px;
    background: none;
    color: #4b6277;
  }
  .rail-icon:hover {
    background: var(--rd-muted);
  }
  .rail-icon[aria-pressed='true'] {
    background: #e8f2fd;
    color: #205b96;
  }
  .pending-dot {
    position: absolute;
    top: 3px;
    right: 3px;
    width: 6px;
    height: 6px;
    border-radius: 999px;
    background: var(--rd-primary);
  }
  .session-row > .pending-dot {
    position: static;
    flex: 0 0 auto;
    margin-left: 6px;
  }
  .new-session-row {
    flex-shrink: 0;
    padding: 0 8px 10px;
  }
  .new-session {
    display: flex;
    align-items: center;
    gap: 7px;
    min-height: 32px;
    padding: 0 10px;
    border: 1px solid #b9cedd;
    border-radius: 6px;
    background: #fff;
    color: #24506f;
    font-size: 11.5px;
    font-weight: 550;
  }
  .app-body.rail-collapsed .new-session {
    justify-content: center;
    padding: 0;
  }
  .search-row {
    flex-shrink: 0;
    padding: 0 8px 8px;
  }
  .search-row input {
    width: 100%;
    min-height: 30px;
    padding: 4px 9px;
    border: 1px solid var(--rd-border);
    border-radius: 6px;
    background: #fff;
    color: #20334b;
    font: inherit;
    font-size: 11px;
  }
  .rail-label {
    flex-shrink: 0;
    margin: 0;
    padding: 0 14px 4px;
    color: var(--rd-muted-foreground);
    font-size: 10px;
    font-weight: 550;
  }
  .project-list {
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: 0 8px 8px;
  }
  .project + .project {
    margin-top: 6px;
  }
  .project-head {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-width: 0;
    height: 30px;
    padding: 0 6px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: var(--rd-muted-foreground);
    font-size: 11px;
    text-align: left;
  }
  .project-head:hover {
    background: #eef4f9;
  }
  .project-head .chevron {
    margin-left: auto;
    transition: transform 160ms ease;
  }
  .chevron.open {
    transform: rotate(0deg);
  }
  .chevron:not(.open) {
    transform: rotate(-90deg);
  }
  .session-row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    min-width: 0;
    min-height: 30px;
    margin-top: 1px;
    padding: 4px 8px 4px 30px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: #29415d;
    font-size: 11px;
    text-align: left;
  }
  .session-row:hover {
    background: #eef4f9;
  }
  .session-row[aria-current='page'] {
    background: #e8f2fd;
    color: #205b96;
    font-weight: 550;
  }
  .session-row.collapsed {
    justify-content: center;
    padding: 6px 0;
  }
  .host-mark {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    color: var(--tone, var(--rd-primary));
  }
  .host-mark svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linejoin: round;
  }
  .rail-foot {
    flex-shrink: 0;
    padding: 8px;
    border-top: 1px solid var(--rd-border);
  }
  .settings-button {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 32px;
    padding: 0 10px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: #29415d;
    font-size: 11.5px;
  }
  .settings-button:hover {
    background: var(--rd-muted);
  }
  .settings-button.active {
    background: #e8f2fd;
    color: #205b96;
    font-weight: 550;
  }
  .app-body.rail-collapsed .settings-button {
    justify-content: center;
    padding: 0;
  }

  /* Request rail */
  .request-rail {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    border-right: 1px solid var(--rd-border);
    background: #fff;
  }
  .requests-head {
    gap: 6px;
    border-bottom: 1px solid var(--rd-border);
  }
  .requests-head strong {
    flex: 0 0 auto;
    font-size: 12px;
  }
  .requests-head .scope {
    min-width: 0;
    flex: 1;
    color: var(--rd-muted-foreground);
    font-size: 10px;
  }
  .count-badge {
    padding: 0 6px;
    border-radius: 999px;
    background: var(--rd-muted);
    color: var(--rd-muted-foreground);
    font-size: 9px;
    font-weight: 550;
    line-height: 16px;
  }
  .filter-panel {
    display: grid;
    gap: 4px;
    flex-shrink: 0;
    padding: 8px 12px;
    border-bottom: 1px solid var(--rd-border);
    background: #f8fafc;
  }
  .filter-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11px;
    color: #33475b;
  }
  .filter-row input {
    width: 14px;
    height: 14px;
    accent-color: var(--rd-primary);
  }
  .request-list {
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
    /* `overflow-y: auto` alone would make this a horizontal scroll container too; the rail must
       never scroll sideways, so the cross axis is pinned. */
    overflow-x: hidden;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: 8px;
  }
  .request-card {
    position: relative;
    display: grid;
    gap: 5px;
    width: 100%;
    min-width: 0;
    padding: 10px 10px 11px;
    border: 0;
    border-bottom: 1px solid var(--rd-muted);
    background: none;
    color: #20334b;
    text-align: left;
  }
  .request-card:hover {
    background: #f6f9fc;
  }
  .request-card[aria-current='true'] {
    background: #e8f2fd;
  }
  .request-card[aria-current='true']::before {
    position: absolute;
    top: 8px;
    bottom: 8px;
    left: 0;
    width: 2px;
    border-radius: 999px;
    background: var(--rd-primary);
    content: '';
  }
  .request-top {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .request-top strong {
    min-width: 0;
    flex: 1;
    font-size: 11.5px;
    font-weight: 550;
  }
  .request-preview {
    display: -webkit-box;
    min-width: 0;
    overflow: hidden;
    color: var(--rd-muted-foreground);
    font-size: 10.5px;
    line-height: 1.55;
    overflow-wrap: anywhere;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
  }
  .request-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    color: var(--rd-muted-foreground);
    font-size: 9px;
  }
  .request-meta .truncate {
    min-width: 0;
    flex: 1;
  }
  .status-badge {
    flex: 0 0 auto;
    padding: 1px 6px;
    border: 1px solid transparent;
    border-radius: 999px;
    font-size: 9px;
    font-weight: 550;
    line-height: 16px;
    white-space: nowrap;
  }
  .status-badge.waiting {
    border-color: #f0d3a5;
    background: #fdf3e4;
    color: #704307;
  }
  .status-badge.in_progress {
    border-color: #bcd6ef;
    background: #eaf3fd;
    color: #205b96;
  }
  .status-badge.completed {
    border-color: #a9d3cf;
    background: #eaf6f4;
    color: #237d78;
  }
  .status-badge.cancelled {
    border-color: #e7bfc2;
    background: #fdeff0;
    color: #c94a52;
  }
  .rail-empty-state {
    display: grid;
    gap: 6px;
    justify-items: center;
    padding: 56px 22px;
    text-align: center;
  }
  .rail-empty-state strong {
    font-size: 11.5px;
  }
  .rail-empty-state span {
    color: var(--rd-muted-foreground);
    font-size: 10.5px;
    line-height: 1.6;
  }
  .empty-icon {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 8px;
    background: var(--rd-muted);
    color: var(--rd-muted-foreground);
  }
  .rail-empty {
    padding: 10px;
    color: var(--rd-muted-foreground);
    font-size: 10.5px;
    line-height: 1.6;
  }

  /* Mobile rails */
  .mobile-strips {
    display: none;
  }

  .product-mock :is(button, input):focus-visible {
    outline: 2px solid var(--rd-primary);
    outline-offset: 1px;
  }

  @container (max-width: 1100px) {
    .app-body,
    .app-body.rail-collapsed {
      grid-template-columns: minmax(0, 1fr);
      overflow-y: auto;
    }
    .host-rail,
    .request-rail {
      display: none;
    }
    .mobile-strips {
      display: grid;
      gap: 6px;
      flex-shrink: 0;
      padding: 8px 10px;
      border-bottom: 1px solid var(--rd-border);
      background: #eef4f9;
    }
    .strip-row {
      display: flex;
      gap: 6px;
      overflow-x: auto;
      padding-bottom: 2px;
    }
    .strip-chip {
      flex: 0 0 auto;
      max-width: 220px;
      min-height: 30px;
      padding: 5px 12px;
      overflow: hidden;
      border: 1px solid var(--rd-border);
      border-radius: 999px;
      background: #fff;
      color: #29415d;
      font-size: 11px;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .strip-chip[aria-current='true'] {
      border-color: #6f9ec1;
      background: #eaf3fa;
      font-weight: 550;
    }
    .app-window {
      /* Stacked layout follows its content instead of the desktop aspect ratio. */
      aspect-ratio: auto;
      height: auto;
      min-height: 0;
    }
  }
  @container (max-width: 720px) {
    .app-frame {
      padding: 0;
      border-radius: 12px;
    }
    .app-window {
      border-radius: 11px;
    }
    .titlebar {
      height: 36px;
    }
    .titlebar-sidebar {
      width: 52px;
      justify-content: center;
      padding: 0 6px;
    }
    .titlebar-sidebar strong {
      display: none;
    }
    .workspace-tab {
      max-width: 150px;
      padding: 0 9px;
    }
    .window-controls span {
      width: 30px;
    }
    .mock-strip {
      align-items: flex-start;
      flex-direction: column;
      gap: 8px;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .titlebar-sidebar,
    .chevron {
      transition: none;
    }
  }
</style>

<script lang="ts">
  // Workspace pane of the mock: the task brief split above the feedback document, plus the
  // agent and settings views the same tabs open in the real app.
  import type { Snippet } from 'svelte'

  import MockCaptureThumb from './MockCaptureThumb.svelte'
  import type {
    DeliveryState,
    DocumentBlock,
    MockRequest,
    MockSession,
    MockUi,
    RequestStatus,
  } from '../content/product-mock'

  interface MockAttachment {
    name: string
    mediaType: string
    sizeKiB: number
  }

  let {
    ui,
    viewKind,
    request = null,
    session = null,
    hostLabel = '',
    hostTone = '#2775ca',
    status = 'waiting',
    delivery = 'none',
    submitting = false,
    briefOpen = true,
    blocks = [],
    attachments = [],
    activeActionId = null,
    editingBlockId = null,
    pendingSpeech = 0,
    tidyBusy = false,
    version = 'uncooked',
    savePhase = 'saved',
    savedRevision = 1,
    previewCapture = null,
    rail = undefined,
    onToggleBrief,
    onSelectAction,
    onEditBlock,
    onCommitBlock,
    onTidy,
    onSetVersion,
    onPreviewAttachment,
  }: {
    ui: MockUi
    viewKind: 'request' | 'agent' | 'settings'
    request?: MockRequest | null
    session?: MockSession | null
    hostLabel?: string
    hostTone?: string
    status?: RequestStatus
    delivery?: DeliveryState
    submitting?: boolean
    briefOpen?: boolean
    blocks?: DocumentBlock[]
    attachments?: MockAttachment[]
    activeActionId?: string | null
    editingBlockId?: string | null
    pendingSpeech?: number
    tidyBusy?: boolean
    version?: 'cooked' | 'uncooked'
    savePhase?: 'saved' | 'saving' | 'unsaved'
    savedRevision?: number
    /** Attachment name the command rail asked to preview; expands the matching capture block. */
    previewCapture?: string | null
    rail?: Snippet
    onToggleBrief?: () => void
    onSelectAction?: (actionId: string, instruction: string) => void
    onEditBlock?: (id: string) => void
    onCommitBlock?: (id: string, text: string) => void
    onTidy?: () => void
    onSetVersion?: (version: 'cooked' | 'uncooked') => void
    onPreviewAttachment?: (name: string) => void
  } = $props()

  let detailsOpen = $state(false)
  let expandedCapture = $state<string | null>(null)
  let settingsSection = $state('appearance')
  let compactRails = $state(true)
  let brandBackground = $state(true)
  let motion = $state(true)

  const readOnly = $derived(status === 'completed' || status === 'cancelled')
  const hasCooked = $derived(Boolean(request?.cooked))
  const showCooked = $derived(hasCooked && version === 'cooked')
  const statusLabel = $derived(ui.requestStatus[status])
  const characterCount = $derived(
    blocks.reduce((total, block) => total + ('text' in block ? block.text.length : 0), 0),
  )
  const saveLabel = $derived(
    savePhase === 'saving'
      ? ui.saving
      : savePhase === 'unsaved'
        ? ui.waitingToAutosave
        : `${ui.saved} · r${savedRevision}`,
  )
  const settingsSections = $derived([
    { id: 'general', title: ui.general },
    { id: 'appearance', title: ui.appearanceTitle },
    { id: 'voice', title: ui.voice },
    { id: 'notifications', title: ui.notifications },
    { id: 'about', title: ui.about },
  ])

  function focusOnMount(node: HTMLElement) {
    if (node instanceof HTMLTextAreaElement) {
      node.focus()
      node.setSelectionRange(node.value.length, node.value.length)
    }
  }

  function mediaLabel(attachment: MockAttachment) {
    if (attachment.mediaType.startsWith('image/')) return 'Image'
    if (attachment.mediaType === 'text/markdown') return 'Markdown'
    return attachment.mediaType.split('/').pop()?.toUpperCase() ?? ui.attachments
  }
</script>

<section class="workspace-panel">
  {#if viewKind === 'settings'}
    <header class="settings-header">
      <h1>{ui.settings}</h1>
      <p>{ui.settingsHint}</p>
    </header>
    <div class="settings-body">
      <nav class="settings-nav" aria-label={ui.settings}>
        {#each settingsSections as section (section.id)}
          <button
            type="button"
            class="settings-tab"
            aria-pressed={settingsSection === section.id}
            onclick={() => (settingsSection = section.id)}
          >
            {section.title}
          </button>
        {/each}
      </nav>
      <div class="settings-panel">
        <h2>{ui.appearanceTitle}</h2>
        <p class="settings-description">{ui.appearanceDescription}</p>
        <div class="settings-row">
          <div>
            <strong>{ui.theme}</strong>
            <small>{ui.themeDetail}</small>
          </div>
          <div class="segmented" role="group" aria-label={ui.theme}>
            <button type="button" aria-pressed="true" class="segment-active">{ui.themeLight}</button>
            <button type="button" aria-pressed="false">{ui.themeDark}</button>
          </div>
        </div>
        <div class="settings-row">
          <div>
            <strong>{ui.rowCompactRails.label}</strong>
            <small>{ui.rowCompactRails.detail}</small>
          </div>
          <label class="switch">
            <input type="checkbox" checked={compactRails} onchange={(event) => (compactRails = event.currentTarget.checked)} />
            <span aria-hidden="true"></span>
          </label>
        </div>
        <div class="settings-row">
          <div>
            <strong>{ui.rowBrandBackground.label}</strong>
            <small>{ui.rowBrandBackground.detail}</small>
          </div>
          <label class="switch">
            <input type="checkbox" checked={brandBackground} onchange={(event) => (brandBackground = event.currentTarget.checked)} />
            <span aria-hidden="true"></span>
          </label>
        </div>
        <div class="settings-row">
          <div>
            <strong>{ui.rowMotion.label}</strong>
            <small>{ui.rowMotion.detail}</small>
          </div>
          <label class="switch">
            <input type="checkbox" checked={motion} onchange={(event) => (motion = event.currentTarget.checked)} />
            <span aria-hidden="true"></span>
          </label>
        </div>
      </div>
    </div>
  {:else if viewKind === 'request' && !request}
    <div class="workspace-empty">
      <img src="/assets/refresh/rambelle-idle.webp" alt="" width="80" height="80" loading="lazy" decoding="async" />
      <strong>{ui.selectRequest}</strong>
      <p>{ui.selectRequestHint}</p>
    </div>
  {:else}
    <header class="workspace-header">
      <div class="header-main">
        <div class="host-line">
          <span class="host-mark" style={`--tone: ${hostTone}`} aria-hidden="true">
            <svg width="10" height="11" viewBox="0 0 24 26"><path d="m12 2 10 6v10l-10 6L2 18V8Z" /></svg>
          </span>
          <span class="host-label">{hostLabel}</span>
          <span aria-hidden="true">/</span>
          <span class="host-session truncate">{request?.hostSessionId ?? session?.id ?? ''}</span>
        </div>
        <div class="title-line">
          {#if request}
            <span class="status-badge {status}">{statusLabel}</span>
            <h1 class="truncate">{request.title || ui.untitledRequest}</h1>
          {:else}
            <span class="status-badge in_progress">{ui.runtimeRunning}</span>
            <h1 class="truncate">{session?.title ?? ''}</h1>
          {/if}
        </div>
      </div>
      <div class="agent-status">
        <div class="acp-line">
          <span class="acp-label">{ui.acp}</span>
          {#if session?.pendingRequests}
            <span class="dot running" aria-hidden="true"></span>
            <span class="truncate">{ui.runtimeRunning}</span>
          {:else}
            <span class="dot" aria-hidden="true"></span>
            <span class="truncate">{ui.runtimeConnected}</span>
          {/if}
          <button type="button" class="link-button" disabled title={ui.viewAgent}>
            {ui.viewAgent}
            <svg class="icon" width="12" height="12" viewBox="0 0 24 24" aria-hidden="true"><path d="M7 17 17 7M9 7h8v8" /></svg>
          </button>
        </div>
        <div class="feedback-line">
          <span class="acp-label">{ui.rambleFeedback}</span>
          <span class="delivery-badge {delivery}">{ui.deliveryState[delivery]}</span>
          {#if request}
            <button type="button" class="link-button" aria-expanded={detailsOpen} onclick={() => (detailsOpen = !detailsOpen)}>
              {ui.details}
              <svg class="icon chevron" class:open={detailsOpen} width="12" height="12" viewBox="0 0 24 24" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg>
            </button>
          {/if}
        </div>
        {#if detailsOpen}
          <p class="delivery-detail">{ui.deliveryDetail[delivery]}</p>
        {/if}
      </div>
    </header>

    {#if viewKind === 'agent'}
      <div class="agent-view">
        {#each session?.messages ?? [] as message, index (index)}
          <article class="message" class:human={message.role === 'human'}>
            <span class="message-meta">{message.meta}</span>
            <p>{message.text}</p>
            {#if message.tools}
              <div class="tool-chips">
                {#each message.tools as tool (tool)}<span class="tool-chip">{tool}</span>{/each}
              </div>
            {/if}
          </article>
        {/each}
        {#if (session?.requestIds.length ?? 0) === 0}
          <p class="empty-note">
            <strong>{ui.agentPending}</strong>
            <span>{ui.agentPendingHint}</span>
          </p>
        {/if}
      </div>
    {:else}
      <div class="workspace-columns">
        <div class="document-column">
          <section class="brief-pane" class:collapsed={!briefOpen}>
            <header class="brief-head">
              <svg class="icon" width="17" height="17" viewBox="0 0 24 24" aria-hidden="true"><path d="m3 6 2 2 3-3M3 14l2 2 3-3M13 7h8M13 15h8" /></svg>
              <strong>{briefOpen ? ui.briefHeadline : ui.taskBrief}</strong>
              {#if request}
                <span class="chip">{ui.steps.replace('{count}', String(request.actions.length))}</span>
                {#if request.agentAttachments.length > 0}
                  <span class="chip attach-chip">
                    <svg class="icon" width="11" height="11" viewBox="0 0 24 24" aria-hidden="true"><path d="M21.4 11.1 12.2 20.3a6 6 0 0 1-8.5-8.5l8.6-8.6a4 4 0 0 1 5.7 5.7l-8.6 8.6a2 2 0 0 1-2.8-2.8l8.5-8.5" /></svg>
                    {request.agentAttachments.length}
                  </span>
                {/if}
              {/if}
              <button type="button" class="icon-button" disabled title={ui.fullscreenPreview} aria-label={ui.fullscreenPreview}>
                <svg class="icon" width="14" height="14" viewBox="0 0 24 24" aria-hidden="true"><path d="M4 9V4h5M20 15v5h-5M15 4h5v5M9 20H4v-5" /></svg>
              </button>
              <button type="button" class="icon-button" aria-label={briefOpen ? ui.collapse : ui.expand} onclick={onToggleBrief}>
                <svg class="icon chevron" class:open={briefOpen} width="14" height="14" viewBox="0 0 24 24" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg>
              </button>
            </header>

            {#if briefOpen}
              <div class="brief-body">
                {#if request}
                  <section>
                    <h2>{ui.whatHappened}</h2>
                    <p>{request.whatHappened}</p>
                  </section>
                  <section>
                    <h2>{ui.actionsToExperience}</h2>
                    <ol class="action-list">
                      {#each request.actions as action, index (action.id)}
                        <li>
                          <button
                            type="button"
                            disabled={readOnly}
                            aria-pressed={activeActionId === action.id}
                            onclick={() => onSelectAction?.(action.id, action.instruction)}
                          >
                            <span class="action-index" aria-hidden="true">{index + 1}</span>
                            <span>{action.instruction}</span>
                          </button>
                        </li>
                      {/each}
                    </ol>
                  </section>
                  {#if request.agentAttachments.length > 0}
                    <section>
                      <h2 class="with-icon">
                        <svg class="icon" width="14" height="14" viewBox="0 0 24 24" aria-hidden="true"><path d="M21.4 11.1 12.2 20.3a6 6 0 0 1-8.5-8.5l8.6-8.6a4 4 0 0 1 5.7 5.7l-8.6 8.6a2 2 0 0 1-2.8-2.8l8.5-8.5" /></svg>
                        {ui.reviewAgentAttachments}
                      </h2>
                      <ul class="agent-attachments">
                        {#each request.agentAttachments as attachment (attachment.id)}
                          <li>
                            <span class="agent-attachment">
                              {#if attachment.mediaType.startsWith('image/')}
                                <svg class="icon" width="14" height="14" viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="3" width="18" height="18" rx="3" /><circle cx="8.5" cy="8.5" r="1.4" /><path d="m3 18 6-6 4 4 3-3 5 5" /></svg>
                              {:else}
                                <svg class="icon" width="14" height="14" viewBox="0 0 24 24" aria-hidden="true"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" /><path d="M14 2v5h5" /></svg>
                              {/if}
                              <span class="agent-attachment-text">
                                <strong class="truncate">{attachment.name}</strong>
                                <small>{attachment.mediaType === 'text/markdown' ? 'Markdown' : 'Image'} · {attachment.sizeKiB.toFixed(1)} KiB</small>
                              </span>
                            </span>
                          </li>
                        {/each}
                      </ul>
                    </section>
                  {/if}
                {:else}
                  <p class="empty-note">{ui.noTaskBrief}</p>
                {/if}
              </div>
            {/if}
          </section>

          <section class="editor-pane">
            <header class="editor-head">
              <div class="editor-title">
                <h2>{ui.feedbackDocument}</h2>
                <p>{readOnly ? ui.documentReadOnly : ui.documentHint}</p>
              </div>
              {#if hasCooked}
                <div class="version-toggle" role="group">
                  <button type="button" aria-pressed={version === 'cooked'} onclick={() => onSetVersion?.('cooked')}>{ui.cooked}</button>
                  <button type="button" aria-pressed={version === 'uncooked'} onclick={() => onSetVersion?.('uncooked')}>{ui.uncooked}</button>
                </div>
              {:else if !readOnly}
                <button type="button" class="tidy-button" disabled={pendingSpeech === 0 || tidyBusy} onclick={onTidy}>
                  <svg class="icon" width="14" height="14" viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3v4M12 17v4M5.6 5.6l2.8 2.8M15.6 15.6l2.8 2.8M3 12h4M17 12h4M5.6 18.4l2.8-2.8M15.6 8.4l2.8-2.8" /></svg>
                  {tidyBusy ? ui.tidying : ui.tidy}
                  {#if pendingSpeech > 0}<span class="chip">{pendingSpeech}</span>{/if}
                </button>
              {/if}
            </header>

            <div class="editor" class:readonly={readOnly}>
              {#if showCooked}
                <p class="block-text">{request?.cooked}</p>
              {:else}
                {#each blocks as block (block.id)}
                  {#if block.kind === 'text'}
                    {#if editingBlockId === block.id}
                      <textarea
                        class="block-input"
                        rows="3"
                        value={block.text}
                        use:focusOnMount
                        onblur={(event) => onCommitBlock?.(block.id, event.currentTarget.value)}
                      ></textarea>
                    {:else if readOnly}
                      <p class="block-text">{block.text}</p>
                    {:else}
                      <button
                        type="button"
                        class="block-text editable"
                        title={ui.clickToEdit}
                        onclick={() => onEditBlock?.(block.id)}
                      >{block.text}</button>
                    {/if}
                  {:else if block.kind === 'speech'}
                    <div class="block-speech" class:pending={block.pending}>
                      <span class="speech-mark" aria-hidden="true">
                        <svg class="icon" width="12" height="12" viewBox="0 0 24 24"><rect x="9" y="2" width="6" height="13" rx="3" /><path d="M5 10v2a7 7 0 0 0 14 0v-2M12 19v3" /></svg>
                      </span>
                      <p>{block.text}</p>
                      {#if block.pending}<span class="chip warning">{ui.pendingSpeech}</span>{/if}
                    </div>
                  {:else if block.kind === 'action'}
                    <blockquote class="block-action">
                      <span class="action-label">{ui.actionGroupLabel}</span>
                      <span>{block.text}</span>
                    </blockquote>
                  {:else}
                    <figure class="block-capture">
                      <button
                        type="button"
                        aria-expanded={expandedCapture === block.id || previewCapture === block.name}
                        onclick={() => (expandedCapture = expandedCapture === block.id ? null : block.id)}
                      >
                        <MockCaptureThumb
                          settings={ui.captureSettings}
                          items={ui.settingsItems}
                          issue={ui.settingsIssue}
                          alt={ui.screenshotAlt}
                          compact={expandedCapture !== block.id && previewCapture !== block.name}
                        />
                      </button>
                      <figcaption>
                        {block.name}
                        <span>· {block.caption}</span>
                      </figcaption>
                    </figure>
                  {/if}
                {/each}
              {/if}

              {#if !readOnly && !hasCooked}
                <div class="bubble-toolbar" aria-hidden="true">
                  <span class="tool-strong">B</span>
                  <span class="tool-italic">I</span>
                  <span class="tool-heading">H2</span>
                  <span class="tool-list">
                    <svg class="icon" width="14" height="14" viewBox="0 0 24 24"><path d="M8 6h13M8 12h13M8 18h13M3.5 6h.01M3.5 12h.01M3.5 18h.01" /></svg>
                  </span>
                  <span class="tool-quote">
                    <svg class="icon" width="14" height="14" viewBox="0 0 24 24"><path d="M7 7h5v5a5 5 0 0 1-5 5M14 7h5v5a5 5 0 0 1-5 5" /></svg>
                  </span>
                  <span class="tool-gap"></span>
                  <span class="tool-undo">
                    <svg class="icon" width="14" height="14" viewBox="0 0 24 24"><path d="M9 14 4 9l5-5" /><path d="M4 9h10a6 6 0 0 1 0 12h-3" /></svg>
                  </span>
                  <span class="tool-redo">
                    <svg class="icon" width="14" height="14" viewBox="0 0 24 24"><path d="m15 14 5-5-5-5" /><path d="M20 9H10a6 6 0 0 0 0 12h3" /></svg>
                  </span>
                </div>
              {/if}
            </div>

            <footer class="editor-foot">
              <span>{ui.characters.replace('{count}', characterCount.toLocaleString())}</span>
              <span>{ui.markdown}</span>
              <span class="save-badge" class:saving={savePhase !== 'saved'}>
                {#if savePhase === 'saved'}
                  <svg class="icon" width="11" height="11" viewBox="0 0 24 24" aria-hidden="true"><path d="m5 12 4 4L19 6" /></svg>
                {:else}
                  <svg class="icon spin" width="11" height="11" viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3a9 9 0 1 0 9 9" /></svg>
                {/if}
                {saveLabel}
              </span>
              <span>{request?.updatedAt ?? ''}</span>
            </footer>
          </section>
        </div>

        {#if rail}{@render rail()}{/if}
      </div>
    {/if}
  {/if}
</section>

<style>
  .workspace-panel {
    --rd-border: #c8d5e3;
    --rd-muted: #edf3f8;
    --rd-muted-foreground: #60738a;
    --rd-primary: #2775ca;
    --rd-success: #237d78;
    --rd-warning: #704307;
    --rd-destructive: #c94a52;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: #f7f9fc;
    color: #20334b;
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
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 1px 7px;
    border-radius: 999px;
    background: var(--rd-muted);
    color: #557190;
    font-size: 9px;
    font-weight: 550;
    line-height: 18px;
    white-space: nowrap;
  }
  .chip.warning {
    background: #fdf2df;
    color: var(--rd-warning);
  }
  .chip.attach-chip {
    border: 1px solid var(--rd-border);
    background: #fff;
  }
  .icon-button {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border: 0;
    border-radius: 5px;
    background: none;
    color: var(--rd-muted-foreground);
  }
  .icon-button:hover:not(:disabled) {
    background: var(--rd-muted);
    color: #29415d;
  }
  .icon-button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .chevron {
    transition: transform 160ms ease;
  }
  .chevron.open {
    transform: rotate(180deg);
  }

  /* Empty workspace */
  .workspace-empty {
    display: grid;
    place-items: center;
    gap: 6px;
    flex: 1 1 auto;
    padding: 32px;
    text-align: center;
  }
  .workspace-empty img {
    width: 80px;
    height: 80px;
    object-fit: contain;
    opacity: 0.9;
  }
  .workspace-empty strong {
    font-size: 13px;
    font-weight: 550;
  }
  .workspace-empty p {
    max-width: 320px;
    margin: 0;
    color: var(--rd-muted-foreground);
    font-size: 11.5px;
    line-height: 1.7;
  }

  /* Workspace header */
  .workspace-header {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 288px;
    flex-shrink: 0;
    min-width: 0;
    border-bottom: 1px solid var(--rd-border);
  }
  .header-main {
    display: flex;
    flex-direction: column;
    justify-content: center;
    min-width: 0;
    min-height: 56px;
    padding: 8px 16px;
  }
  .host-line {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    color: var(--rd-muted-foreground);
    font-size: 10px;
    white-space: nowrap;
  }
  .host-mark {
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    flex: 0 0 auto;
    color: var(--tone, var(--rd-primary));
  }
  .host-mark svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linejoin: round;
  }
  .host-label {
    font-weight: 550;
    color: #20334b;
  }
  .host-session {
    max-width: 12rem;
    font-family: ui-monospace, monospace;
  }
  .title-line {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    margin-top: 4px;
  }
  .title-line h1 {
    margin: 0;
    font-size: 13.5px;
    font-weight: 600;
    line-height: 1.4;
  }
  .status-badge {
    flex: 0 0 auto;
    padding: 1px 7px;
    border: 1px solid transparent;
    border-radius: 999px;
    font-size: 9px;
    font-weight: 550;
    line-height: 18px;
    white-space: nowrap;
  }
  .status-badge.waiting {
    border-color: #f0d3a5;
    background: #fdf3e4;
    color: var(--rd-warning);
  }
  .status-badge.in_progress {
    border-color: #bcd6ef;
    background: #eaf3fd;
    color: #205b96;
  }
  .status-badge.completed {
    border-color: #a9d3cf;
    background: #eaf6f4;
    color: var(--rd-success);
  }
  .status-badge.cancelled {
    border-color: #e7bfc2;
    background: #fdeff0;
    color: var(--rd-destructive);
  }
  .agent-status {
    min-width: 0;
    overflow: hidden;
    padding: 8px 14px;
    border-left: 1px solid var(--rd-border);
    background: #f4f8fc;
    font-size: 11px;
  }
  .acp-line,
  .feedback-line {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    height: 22px;
  }
  .acp-label {
    flex: 0 0 auto;
    color: var(--rd-muted-foreground);
    font-size: 10px;
    font-weight: 550;
  }
  .dot {
    width: 7px;
    height: 7px;
    flex: 0 0 auto;
    border-radius: 999px;
    background: var(--rd-success);
  }
  .dot.running {
    background: var(--rd-primary);
    animation: rd-pulse 1.6s ease-in-out infinite;
  }
  @keyframes rd-pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.35;
    }
  }
  .link-button {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    margin-left: auto;
    padding: 2px 5px;
    border: 0;
    border-radius: 5px;
    background: none;
    color: #205b96;
    font-size: 10px;
    font-weight: 550;
    white-space: nowrap;
  }
  .link-button:hover:not(:disabled) {
    background: var(--rd-muted);
  }
  .link-button:disabled {
    color: var(--rd-muted-foreground);
    cursor: default;
  }
  .delivery-badge {
    min-width: 0;
    overflow: hidden;
    padding: 1px 7px;
    border: 1px solid var(--rd-border);
    border-radius: 999px;
    color: #29415d;
    font-size: 10px;
    line-height: 18px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .delivery-badge.delivered {
    border-color: #a9d3cf;
    background: #eaf6f4;
    color: var(--rd-success);
  }
  .delivery-badge.sending {
    border-color: #bcd6ef;
    background: #eaf3fd;
    color: #205b96;
  }
  .delivery-detail {
    margin: 2px 0 0;
    color: var(--rd-muted-foreground);
    font-size: 10px;
    line-height: 1.55;
  }

  /* Columns */
  .workspace-columns {
    display: grid;
    /* 288px matches the real command rail width at the desktop window's minimum size. */
    grid-template-columns: minmax(0, 1fr) 288px;
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  .document-column {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  /* Task brief */
  .brief-pane {
    flex-shrink: 0;
    max-height: 46%;
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-bottom: 1px solid var(--rd-border);
    background: #fff;
  }
  .brief-pane.collapsed {
    max-height: none;
  }
  .brief-head {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 44px;
    padding: 6px 14px;
    flex-shrink: 0;
  }
  .brief-head > .icon {
    color: var(--rd-muted-foreground);
  }
  .brief-head strong {
    font-size: 11.5px;
    font-weight: 600;
  }
  .brief-head .icon-button:first-of-type {
    margin-left: auto;
  }
  .brief-body {
    display: grid;
    gap: 18px;
    min-width: 0;
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: 4px 16px 18px;
    background: #f8fafc;
  }
  .brief-body h2 {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 0 8px;
    padding-bottom: 6px;
    border-bottom: 1px solid var(--rd-border);
    font-size: 12px;
    font-weight: 600;
  }
  .brief-body section > p {
    margin: 0;
    font-size: 12.5px;
    line-height: 1.7;
    color: #35485c;
  }
  .action-list {
    display: grid;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .action-list button {
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr);
    gap: 8px;
    width: 100%;
    padding: 6px 7px;
    border: 1px solid transparent;
    border-radius: 6px;
    background: none;
    color: #2f4759;
    text-align: left;
    font-size: 12.5px;
    line-height: 1.65;
  }
  .action-list button:hover:not(:disabled) {
    background: color-mix(in srgb, var(--rd-primary) 7%, #fff);
  }
  .action-list button[aria-pressed='true'] {
    border-color: color-mix(in srgb, var(--rd-primary) 30%, #fff);
    background: color-mix(in srgb, var(--rd-primary) 10%, #fff);
  }
  .action-list button:disabled {
    color: #6f8496;
    cursor: default;
  }
  .action-index {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    margin-top: 1px;
    border: 1px solid var(--rd-border);
    border-radius: 6px;
    background: #fff;
    color: var(--rd-muted-foreground);
    font-size: 10px;
    font-weight: 600;
  }
  .agent-attachments {
    display: grid;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .agent-attachment {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 7px 10px;
    border: 1px solid var(--rd-border);
    border-radius: 8px;
    background: #fff;
  }
  .agent-attachment > .icon {
    color: var(--rd-muted-foreground);
  }
  .agent-attachment-text {
    min-width: 0;
    flex: 1;
  }
  .agent-attachment-text strong {
    display: block;
    font-size: 11.5px;
    font-weight: 550;
  }
  .agent-attachment-text small {
    display: block;
    color: var(--rd-muted-foreground);
    font-size: 10px;
  }

  /* Editor */
  .editor-pane {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    min-height: 0;
    padding: 16px 18px 8px;
  }
  .editor-head {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    margin-bottom: 10px;
  }
  .editor-title {
    min-width: 0;
    flex: 1;
  }
  .editor-title h2 {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
  }
  .editor-title p {
    margin: 2px 0 0;
    color: var(--rd-muted-foreground);
    font-size: 10px;
    line-height: 1.5;
  }
  .tidy-button,
  .version-toggle button {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    min-height: 26px;
    padding: 3px 9px;
    border: 1px solid var(--rd-border);
    border-radius: 6px;
    background: #fff;
    color: #29415d;
    font-size: 10px;
    font-weight: 550;
    white-space: nowrap;
  }
  .tidy-button:hover:not(:disabled) {
    background: var(--rd-muted);
  }
  .tidy-button:disabled {
    color: #93a6b4;
    cursor: default;
  }
  .version-toggle {
    display: inline-flex;
    gap: 3px;
    padding: 2px;
    border: 1px solid var(--rd-border);
    border-radius: 7px;
    background: #f1f5f9;
  }
  .version-toggle button {
    border: 0;
    background: none;
  }
  .version-toggle button[aria-pressed='true'] {
    background: #fff;
    box-shadow: 0 1px 2px rgb(15 35 55 / 12%);
  }
  .editor {
    position: relative;
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: 12px 14px 18px;
    border: 1px solid var(--rd-border);
    border-radius: 8px;
    background: #fff;
  }
  .editor.readonly {
    background: #fbfcfe;
  }
  .block-text {
    margin: 0 0 11px;
    font-size: 13.5px;
    line-height: 1.85;
    color: #26384a;
  }
  .block-text.editable {
    display: block;
    width: 100%;
    padding: 2px 6px;
    margin: 0 0 11px -6px;
    border: 0;
    border-radius: 5px;
    background: none;
    color: #26384a;
    font: inherit;
    font-size: 13.5px;
    line-height: 1.85;
    text-align: left;
    cursor: text;
  }
  .block-text.editable:hover {
    background: color-mix(in srgb, var(--rd-primary) 6%, #fff);
  }
  .block-input {
    width: 100%;
    margin: 0 0 11px;
    padding: 6px 8px;
    border: 1px solid var(--rd-primary);
    border-radius: 6px;
    background: #fff;
    color: #26384a;
    font: inherit;
    font-size: 13.5px;
    line-height: 1.85;
    resize: vertical;
  }
  .block-speech {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: 0 0 11px;
    padding: 8px 10px;
    border: 1px solid var(--rd-border);
    border-left: 3px solid #9fb8cc;
    border-radius: 6px;
    background: #f4f8fc;
  }
  .block-speech.pending {
    border-left-color: #e0912c;
    background: #fdf6ea;
  }
  .block-speech p {
    margin: 0;
    min-width: 0;
    flex: 1;
    font-size: 13.5px;
    line-height: 1.8;
    color: #2b3d50;
  }
  .speech-mark {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 20px;
    height: 20px;
    margin-top: 2px;
    border-radius: 999px;
    background: #fff;
    color: var(--rd-muted-foreground);
  }
  .block-action {
    display: grid;
    gap: 3px;
    margin: 0 0 11px;
    padding: 8px 12px;
    border-left: 3px solid #7ba7c8;
    border-radius: 0 6px 6px 0;
    background: #f3f8fc;
    font-size: 13px;
    line-height: 1.75;
    color: #2d4256;
  }
  .action-label {
    color: #4a7291;
    font-size: 10px;
    font-weight: 650;
    font-family: ui-monospace, monospace;
  }
  .block-capture {
    margin: 0 0 14px;
  }
  .block-capture button {
    display: block;
    max-width: 260px;
    padding: 7px;
    border: 1px solid var(--rd-border);
    border-radius: 8px;
    background: #f7fafc;
  }
  .block-capture figcaption {
    margin-top: 6px;
    color: var(--rd-muted-foreground);
    font-size: 10px;
  }
  .block-capture figcaption span {
    color: #8fa3b3;
  }
  .bubble-toolbar {
    position: absolute;
    top: 8px;
    right: 10px;
    display: inline-flex;
    align-items: center;
    gap: 2px;
    padding: 3px;
    border: 1px solid var(--rd-border);
    border-radius: 8px;
    background: #fff;
    box-shadow: 0 4px 12px rgb(20 40 60 / 12%);
    color: #4d6377;
    pointer-events: none;
  }
  .bubble-toolbar span {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 5px;
    font-size: 11px;
  }
  .bubble-toolbar .tool-italic {
    font-style: italic;
  }
  .bubble-toolbar .tool-heading {
    font-size: 10px;
    font-weight: 650;
  }
  .tool-gap {
    width: 8px;
  }
  .editor-foot {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 6px;
    color: var(--rd-muted-foreground);
    font-size: 9px;
  }
  .save-badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-left: auto;
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--rd-muted);
    font-size: 9px;
  }
  .save-badge.saving {
    background: #eaf3fd;
    color: #205b96;
  }
  .spin {
    animation: rd-spin 900ms linear infinite;
  }
  @keyframes rd-spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Agent view */
  .agent-view {
    display: grid;
    gap: 14px;
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: 20px 22px;
  }
  .message {
    padding-left: 12px;
    border-left: 2px solid #c3d6e4;
  }
  .message.human {
    border-left-color: #7ba7c8;
  }
  .message-meta {
    display: block;
    margin-bottom: 4px;
    color: #7b92a3;
    font-size: 10px;
    font-family: ui-monospace, monospace;
  }
  .message p {
    margin: 0;
    font-size: 13px;
    line-height: 1.75;
    color: #2c4356;
  }
  .tool-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 8px;
  }
  .tool-chip {
    padding: 3px 7px;
    border: 1px solid var(--rd-border);
    border-radius: 5px;
    background: #f5f9fc;
    color: #46637a;
    font-size: 10px;
    font-family: ui-monospace, monospace;
  }
  .empty-note {
    display: grid;
    gap: 4px;
    max-width: 460px;
    margin: 0;
    color: var(--rd-muted-foreground);
    font-size: 12px;
    line-height: 1.7;
  }
  .empty-note strong {
    color: #29415d;
    font-size: 12.5px;
  }

  /* Settings view */
  .settings-header {
    flex-shrink: 0;
    padding: 16px 20px 12px;
    border-bottom: 1px solid var(--rd-border);
    background: #fff;
  }
  .settings-header h1 {
    margin: 0;
    font-size: 15px;
    font-weight: 650;
  }
  .settings-header p {
    margin: 4px 0 0;
    color: var(--rd-muted-foreground);
    font-size: 10.5px;
  }
  .settings-body {
    display: grid;
    grid-template-columns: 190px minmax(0, 1fr);
    flex: 1 1 auto;
    min-height: 0;
    overflow: hidden;
  }
  .settings-nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px;
    border-right: 1px solid var(--rd-border);
    background: #f4f8fc;
  }
  .settings-tab {
    padding: 8px 10px;
    border: 1px solid transparent;
    border-radius: 6px;
    background: none;
    color: #29415d;
    text-align: left;
    font-size: 11.5px;
  }
  .settings-tab:hover {
    background: #fff;
  }
  .settings-tab[aria-pressed='true'] {
    border-color: #bcd6ef;
    background: #eaf3fd;
    color: #205b96;
    font-weight: 550;
  }
  .settings-panel {
    min-width: 0;
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
    padding: 18px 20px;
  }
  .settings-panel h2 {
    margin: 0;
    font-size: 14px;
    font-weight: 650;
  }
  .settings-description {
    margin: 4px 0 14px;
    color: var(--rd-muted-foreground);
    font-size: 10.5px;
  }
  .settings-row {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 12px 0;
    border-top: 1px solid var(--rd-muted);
  }
  .settings-row > div:first-child {
    min-width: 0;
    flex: 1;
  }
  .settings-row strong {
    display: block;
    font-size: 12px;
    font-weight: 550;
  }
  .settings-row small {
    display: block;
    margin-top: 3px;
    color: var(--rd-muted-foreground);
    font-size: 10px;
    line-height: 1.6;
  }
  .segmented {
    display: inline-flex;
    gap: 2px;
    padding: 2px;
    border: 1px solid var(--rd-border);
    border-radius: 7px;
    background: #f1f5f9;
  }
  .segmented button {
    min-height: 24px;
    padding: 3px 10px;
    border: 0;
    border-radius: 5px;
    background: none;
    color: #4d6377;
    font-size: 10.5px;
  }
  .segmented button.segment-active {
    background: #fff;
    box-shadow: 0 1px 2px rgb(15 35 55 / 12%);
    font-weight: 550;
  }
  .switch {
    position: relative;
    display: inline-flex;
    align-items: center;
    flex: 0 0 auto;
    cursor: pointer;
  }
  .switch input {
    position: absolute;
    width: 34px;
    height: 20px;
    opacity: 0;
  }
  .switch span {
    position: relative;
    width: 34px;
    height: 20px;
    border-radius: 999px;
    background: #cbd8e4;
    transition: background 160ms ease;
  }
  .switch span::after {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 999px;
    background: #fff;
    box-shadow: 0 1px 2px rgb(15 35 55 / 25%);
    content: '';
    transition: transform 160ms ease;
  }
  .switch input:checked + span {
    background: var(--rd-primary);
  }
  .switch input:checked + span::after {
    transform: translateX(14px);
  }
  .switch input:focus-visible + span {
    outline: 2px solid var(--rd-primary);
    outline-offset: 2px;
  }

  @container (max-width: 1100px) {
    .workspace-header {
      grid-template-columns: minmax(0, 1fr);
    }
    .agent-status {
      border-top: 1px solid var(--rd-border);
      border-left: 0;
    }
    .workspace-columns {
      grid-template-columns: minmax(0, 1fr);
      overflow-y: auto;
    }
    .document-column {
      min-height: 460px;
    }
  }
  @container (max-width: 720px) {
    .settings-body {
      grid-template-columns: minmax(0, 1fr);
    }
    .settings-nav {
      flex-direction: row;
      overflow-x: auto;
      border-right: 0;
      border-bottom: 1px solid var(--rd-border);
    }
    .settings-tab {
      white-space: nowrap;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .dot.running,
    .spin {
      animation: none;
    }
    .chevron,
    .switch span,
    .switch span::after {
      transition: none;
    }
  }
</style>

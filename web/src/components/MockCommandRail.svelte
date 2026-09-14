<script lang="ts">
  // Right rail of the mock, mirroring the real workbench command rail:
  // Ramble, Add context, Attachments, Feedback Package, Rambelle status.
  import type { DeliveryState, MockUi, RequestStatus } from '../content/product-mock'

  interface MockAttachment {
    name: string
    mediaType: string
    sizeKiB: number
  }

  let {
    ui,
    status,
    delivery,
    attachments = [],
    ramblePhase = 'idle',
    segmentCount = 0,
    submitting = false,
    packageOpen = false,
    packageFiles = [],
    onToggleRecording,
    onCapture,
    onClipboard,
    onFiles,
    onRemoveAttachment,
    onPreviewAttachment,
    onSubmit,
    onCancel,
    onTogglePackage,
  }: {
    ui: MockUi
    status: RequestStatus
    delivery: DeliveryState
    attachments?: MockAttachment[]
    ramblePhase?: 'idle' | 'recording'
    segmentCount?: number
    submitting?: boolean
    packageOpen?: boolean
    packageFiles?: { name: string; note: string }[]
    onToggleRecording: () => void
    onCapture: () => void
    onClipboard: () => void
    onFiles: () => void
    onRemoveAttachment: (name: string) => void
    onPreviewAttachment: (name: string) => void
    onSubmit: () => void
    onCancel: () => void
    onTogglePackage: () => void
  } = $props()

  const readOnly = $derived(status === 'completed' || status === 'cancelled')
  const published = $derived(status === 'completed')
  const recording = $derived(ramblePhase === 'recording')
  const rambellePortrait = $derived(
    published ? '/assets/refresh/rambelle-archived.webp' : recording ? '/assets/refresh/rambelle-recording.webp' : '/assets/refresh/rambelle-idle.webp',
  )
  const rambelleLine = $derived(
    published
      ? ui.rambelleLines.published
      : status === 'cancelled'
        ? ui.rambelleLines.cancelled
        : recording
          ? ui.rambelleLines.recording
          : ui.rambelleLines.idle,
  )

  function mediaLabel(attachment: MockAttachment) {
    if (attachment.mediaType.startsWith('image/')) return 'Image'
    if (attachment.mediaType === 'text/markdown') return 'Markdown'
    return attachment.mediaType.split('/').pop()?.toUpperCase() ?? ui.attachments
  }
</script>

<aside class="command-rail" aria-label={ui.ramble}>
  {#if !readOnly}
    <section class="card">
      <header class="card-head">
        <svg class="icon" width="19" height="19" viewBox="0 0 24 24" aria-hidden="true"><rect x="9" y="2" width="6" height="13" rx="3" /><path d="M5 10v2a7 7 0 0 0 14 0v-2M12 19v3" /></svg>
        <strong>{ui.ramble}</strong>
        <span class="chip" class:active={recording}>{recording ? ui.recording : ui.standby}</span>
      </header>
      <button type="button" class="ramble-button" class:active={recording} aria-pressed={recording} onclick={onToggleRecording}>
        {#if recording}<span class="record-led" aria-hidden="true"></span>{/if}
        <svg class="icon" width="15" height="15" viewBox="0 0 24 24" aria-hidden="true"><rect x="9" y="2" width="6" height="13" rx="3" /><path d="M5 10v2a7 7 0 0 0 14 0v-2M12 19v3" /></svg>
        {recording ? ui.stopRecording : ui.startRecording}
      </button>
      <div class="ramble-meta">
        <div class="device-line">
          <span class={recording ? 'record-led' : 'idle-dot'} aria-hidden="true"></span>
          <span class="truncate">{ui.defaultMicrophone}</span>
          {#if segmentCount > 0}<span class="tabular">{ui.segments.replace('{count}', String(segmentCount))}</span>{/if}
        </div>
        <p>{ui.transcriptNote}</p>
        <div class="level" aria-hidden="true"><span class:active={recording}></span></div>
      </div>
    </section>

    <section class="card">
      <header class="card-head">
        <svg class="icon" width="15" height="15" viewBox="0 0 24 24" aria-hidden="true"><path d="M21.4 11.1 12.2 20.3a6 6 0 0 1-8.5-8.5l8.6-8.6a4 4 0 0 1 5.7 5.7l-8.6 8.6a2 2 0 0 1-2.8-2.8l8.5-8.5" /></svg>
        <strong>{ui.addContext}</strong>
        <span class="counter">{attachments.length}</span>
      </header>
      <div class="capture-grid">
        <button type="button" onclick={onCapture}>
          <svg class="icon" width="16" height="16" viewBox="0 0 24 24" aria-hidden="true"><path d="M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3l-2.5-3Z" /><circle cx="12" cy="13" r="3" /></svg>
          <span>{ui.capture}</span>
        </button>
        <button type="button" onclick={onClipboard}>
          <svg class="icon" width="16" height="16" viewBox="0 0 24 24" aria-hidden="true"><path d="M15 4h2a2 2 0 0 1 2 2v13a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" /><rect x="9" y="2" width="6" height="4" rx="1" /><path d="M9 13h6M9 17h4" /></svg>
          <span>{ui.clipboard}</span>
        </button>
        <button type="button" onclick={onFiles}>
          <svg class="icon" width="16" height="16" viewBox="0 0 24 24" aria-hidden="true"><path d="M21.4 11.1 12.2 20.3a6 6 0 0 1-8.5-8.5l8.6-8.6a4 4 0 0 1 5.7 5.7l-8.6 8.6a2 2 0 0 1-2.8-2.8l8.5-8.5" /></svg>
          <span>{ui.files}</span>
        </button>
      </div>
      <p class="fine">{ui.clipboardNote}</p>
    </section>
  {/if}

  <section class="card attachments-card">
    <header class="card-head">
      <svg class="icon" width="15" height="15" viewBox="0 0 24 24" aria-hidden="true"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" /><path d="M14 2v5h5M9 13h6M9 17h4" /></svg>
      <strong>{ui.attachments}</strong>
      <span class="chip">{attachments.length}</span>
    </header>
    {#if attachments.length > 0}
      <ul class="attachment-list">
        {#each attachments as attachment (attachment.name)}
          <li>
            {#if attachment.mediaType.startsWith('image/')}
              <svg class="icon" width="14" height="14" viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="3" width="18" height="18" rx="3" /><circle cx="8.5" cy="8.5" r="1.4" /><path d="m3 18 6-6 4 4 3-3 5 5" /></svg>
            {:else}
              <svg class="icon" width="14" height="14" viewBox="0 0 24 24" aria-hidden="true"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" /><path d="M14 2v5h5" /></svg>
            {/if}
            <span class="attachment-text">
              <strong class="truncate">{attachment.name}</strong>
              <small>{mediaLabel(attachment)} · {attachment.sizeKiB.toFixed(1)} KiB</small>
            </span>
            <button type="button" class="icon-button" aria-label={`${ui.preview} · ${attachment.name}`} onclick={() => onPreviewAttachment(attachment.name)}>
              <svg class="icon" width="14" height="14" viewBox="0 0 24 24" aria-hidden="true"><path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7-10-7-10-7Z" /><circle cx="12" cy="12" r="3" /></svg>
            </button>
            <button type="button" class="icon-button danger" aria-label={`${ui.delete} · ${attachment.name}`} disabled={readOnly} onclick={() => onRemoveAttachment(attachment.name)}>
              <svg class="icon" width="14" height="14" viewBox="0 0 24 24" aria-hidden="true"><path d="M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14" /></svg>
            </button>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="fine">{ui.noAttachments}</p>
    {/if}
  </section>

  <section class="card delivery-card">
    {#if published}
      <header class="card-head">
        <strong>{ui.feedbackPackage}</strong>
        <span class="chip success">
          <svg class="icon" width="12" height="12" viewBox="0 0 24 24" aria-hidden="true"><path d="m5 12 4 4L19 6" /></svg>
          {ui.published}
        </span>
      </header>
      <button type="button" class="package-button" aria-expanded={packageOpen} onclick={onTogglePackage}>
        <svg class="icon" width="15" height="15" viewBox="0 0 24 24" aria-hidden="true"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" /></svg>
        {ui.openPackage}
      </button>
      {#if packageOpen}
        <ul class="package-files">
          {#each packageFiles as file (file.name)}
            <li><strong>{file.name}</strong> — {file.note}</li>
          {/each}
        </ul>
      {/if}
    {:else if status === 'cancelled'}
      <header class="card-head">
        <strong>{ui.feedbackPackage}</strong>
      </header>
      <span class="chip danger">{ui.cancelled}</span>
      <p class="fine">{ui.cancelledNote}</p>
    {:else}
      <div class="delivery-actions">
        <button type="button" class="submit-button" disabled={submitting} onclick={onSubmit}>
          {#if submitting}
            <svg class="icon spin" width="15" height="15" viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3a9 9 0 1 0 9 9" /></svg>
          {:else}
            <svg class="icon" width="15" height="15" viewBox="0 0 24 24" aria-hidden="true"><path d="M4 12h13M11 5l7 7-7 7" /></svg>
          {/if}
          {submitting ? ui.publishing : ui.submitFeedback}
        </button>
        <button type="button" class="cancel-button" disabled={submitting} onclick={onCancel}>
          <svg class="icon" width="15" height="15" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9" /><path d="m6 6 12 12" /></svg>
          {ui.cancelFeedback}
        </button>
      </div>
      <p class="fine">{ui.deliveryDetail[delivery]}</p>
    {/if}
  </section>

  <section class="rambelle" aria-label={ui.rambelleStatus}>
    <img src={rambellePortrait} alt="Rambelle" width="120" height="120" loading="lazy" decoding="async" />
    <div class="bubble">
      <strong>{ui.rambelle}</strong>
      <p>{rambelleLine}</p>
    </div>
  </section>
</aside>

<style>
  .command-rail {
    --rd-border: #c8d5e3;
    --rd-muted: #edf3f8;
    --rd-muted-foreground: #60738a;
    --rd-primary: #2775ca;
    --rd-success: #237d78;
    --rd-destructive: #c94a52;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    border-left: 1px solid var(--rd-border);
    background: #f2f7fb;
    /* A vertical scroll container is also a cross-axis one unless it is pinned. */
    overflow-x: hidden;
    overflow-y: auto;
    overscroll-behavior: contain;
  }
  .card {
    flex-shrink: 0;
    padding: 14px;
    border-bottom: 1px solid var(--rd-border);
  }
  .attachments-card {
    flex: 1 1 auto;
    min-height: 0;
  }
  .card-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 10px;
  }
  .card-head strong {
    font-size: 12px;
    font-weight: 550;
  }
  .card-head .chip,
  .card-head .counter {
    margin-left: auto;
  }
  .icon {
    flex: 0 0 auto;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .card-head > .icon {
    color: var(--rd-muted-foreground);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 1px 7px;
    border-radius: 999px;
    background: var(--rd-muted);
    color: var(--rd-muted-foreground);
    font-size: 9px;
    font-weight: 550;
    line-height: 18px;
    white-space: nowrap;
  }
  .chip.active {
    background: var(--rd-destructive);
    color: #fff;
  }
  .chip.success {
    background: color-mix(in srgb, var(--rd-success) 16%, #fff);
    color: var(--rd-success);
  }
  .chip.danger {
    background: color-mix(in srgb, var(--rd-destructive) 14%, #fff);
    color: var(--rd-destructive);
  }
  .counter {
    color: var(--rd-muted-foreground);
    font-size: 10px;
    font-variant-numeric: tabular-nums;
  }
  .ramble-button {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    width: 100%;
    min-height: 36px;
    border: 1px solid var(--rd-primary);
    border-radius: 6px;
    background: var(--rd-primary);
    color: #fff;
    font-size: 12px;
    font-weight: 550;
  }
  .ramble-button.active {
    border-color: var(--rd-destructive);
    background: var(--rd-destructive);
  }
  .record-led {
    display: inline-block;
    width: 8px;
    height: 8px;
    flex: 0 0 auto;
    border-radius: 999px;
    background: #fff;
    box-shadow: 0 0 6px rgb(255 255 255 / 65%);
    animation: rd-record-blink 1s ease-in-out infinite;
  }
  .idle-dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    flex: 0 0 auto;
    border-radius: 999px;
    background: color-mix(in srgb, var(--rd-muted-foreground) 40%, transparent);
  }
  @keyframes rd-record-blink {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.28;
    }
  }
  .ramble-meta {
    margin-top: 11px;
    color: var(--rd-muted-foreground);
    font-size: 10px;
    line-height: 1.6;
  }
  .device-line {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .truncate {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tabular {
    margin-left: auto;
    font-variant-numeric: tabular-nums;
  }
  .ramble-meta p {
    margin: 4px 0 0;
  }
  .level {
    height: 4px;
    margin-top: 8px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--rd-muted);
  }
  .level span {
    display: block;
    width: 12%;
    height: 100%;
    background: var(--rd-primary);
    transition: width 220ms ease;
  }
  .level span.active {
    animation: rd-level 1.4s ease-in-out infinite;
  }
  @keyframes rd-level {
    0%,
    100% {
      width: 18%;
    }
    35% {
      width: 72%;
    }
    70% {
      width: 44%;
    }
  }
  .capture-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 6px;
  }
  .capture-grid button {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    padding: 10px 4px;
    border: 1px solid var(--rd-border);
    border-radius: 6px;
    background: #fff;
    color: #29415d;
    font-size: 10px;
  }
  .capture-grid button:hover {
    border-color: color-mix(in srgb, var(--rd-primary) 45%, var(--rd-border));
    background: color-mix(in srgb, var(--rd-primary) 6%, #fff);
  }
  .fine {
    margin: 8px 0 0;
    color: var(--rd-muted-foreground);
    font-size: 9px;
    line-height: 1.7;
  }
  .attachment-list {
    display: grid;
    gap: 0;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .attachment-list li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 0;
    border-top: 1px solid var(--rd-muted);
  }
  .attachment-list li:first-child {
    border-top: 0;
    padding-top: 0;
  }
  .attachment-list > li > .icon {
    color: var(--rd-muted-foreground);
  }
  .attachment-text {
    min-width: 0;
    flex: 1;
  }
  .attachment-text strong {
    display: block;
    font-size: 10px;
    font-weight: 550;
    overflow-wrap: anywhere;
  }
  .attachment-text small {
    display: block;
    color: var(--rd-muted-foreground);
    font-size: 9px;
  }
  .icon-button {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: 5px;
    background: none;
    color: var(--rd-muted-foreground);
  }
  .icon-button:hover:not(:disabled) {
    background: var(--rd-muted);
    color: #29415d;
  }
  .icon-button.danger:hover:not(:disabled) {
    color: var(--rd-destructive);
  }
  .icon-button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .delivery-actions {
    display: grid;
    gap: 7px;
  }
  .submit-button {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    min-height: 34px;
    border: 1px solid var(--rd-primary);
    border-radius: 6px;
    background: var(--rd-primary);
    color: #fff;
    font-size: 12px;
    font-weight: 550;
  }
  .submit-button:disabled {
    opacity: 0.65;
    cursor: default;
  }
  .cancel-button {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    min-height: 34px;
    border: 1px solid color-mix(in srgb, var(--rd-destructive) 45%, #fff);
    border-radius: 6px;
    background: #fff;
    color: var(--rd-destructive);
    font-size: 12px;
  }
  .cancel-button:hover:not(:disabled) {
    background: color-mix(in srgb, var(--rd-destructive) 7%, #fff);
  }
  .package-button {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    width: 100%;
    min-height: 34px;
    border: 1px solid var(--rd-border);
    border-radius: 6px;
    background: #fff;
    color: #29415d;
    font-size: 12px;
    font-weight: 550;
  }
  .package-button:hover {
    border-color: color-mix(in srgb, var(--rd-primary) 45%, var(--rd-border));
    background: color-mix(in srgb, var(--rd-primary) 6%, #fff);
  }
  .package-files li {
    overflow-wrap: anywhere;
  }
  .package-files {
    margin: 8px 0 0;
    padding: 0 0 0 16px;
    color: var(--rd-muted-foreground);
    font-size: 10px;
    line-height: 1.8;
    font-family: ui-monospace, monospace;
  }
  .rambelle {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    flex-shrink: 0;
    min-height: 136px;
    padding: 10px 12px;
    border-top: 1px solid var(--rd-border);
    background: color-mix(in srgb, var(--rd-muted) 60%, #fff);
  }
  .rambelle img {
    width: 120px;
    height: 120px;
    flex: 0 0 auto;
    object-fit: contain;
    object-position: top;
  }
  .bubble {
    position: relative;
    min-width: 0;
    flex: 1;
    margin-top: 12px;
    padding: 8px 11px;
    border: 1px solid var(--rd-border);
    border-radius: 16px;
    background: #fff;
    filter: drop-shadow(0 1px 2px rgb(15 35 55 / 12%));
  }
  .bubble::before {
    position: absolute;
    bottom: 15px;
    left: -7px;
    width: 12px;
    height: 12px;
    border-bottom: 1px solid var(--rd-border);
    border-left: 1px solid var(--rd-border);
    background: #fff;
    content: '';
    transform: rotate(45deg);
  }
  .bubble strong {
    display: block;
    font-size: 10px;
    font-weight: 600;
  }
  .bubble p {
    margin: 4px 0 0;
    font-size: 11px;
    line-height: 1.5;
    overflow-wrap: anywhere;
  }
  .spin {
    animation: rd-spin 900ms linear infinite;
  }
  @keyframes rd-spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (pointer: coarse) {
    .icon-button {
      width: 34px;
      height: 34px;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .record-led,
    .level span.active,
    .spin {
      animation: none;
    }
  }
</style>

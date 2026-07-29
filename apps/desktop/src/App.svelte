<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import {
    isPermissionGranted,
    requestPermission,
    sendNotification,
  } from '@tauri-apps/plugin-notification'
  import { onMount } from 'svelte'

  import type {
    DraftView,
    FeedbackRequestSummary,
    FeedbackRequestView,
    FeedbackWorkspaceView,
    SaveDraftInput,
    SubmitFeedbackInput,
  } from './lib/feedback'
  import { requestStatusLabel } from './lib/feedback'
  import type { HealthSnapshot } from './lib/generated/health'
  import {
    InboxNotificationTracker,
    notificationLabel,
    type NotificationState,
  } from './lib/notifications'

  type SavePhase = 'idle' | 'unsaved' | 'saving' | 'saved' | 'error'
  type CommandError = { code: string; message: string; retryable: boolean }

  let health: HealthSnapshot | null = null
  let endpoint = '正在连接…'
  let inbox: FeedbackRequestSummary[] = []
  let workspace: FeedbackWorkspaceView | null = null
  let completedResult: FeedbackRequestView | null = null
  let draftBody = ''
  let savedBody = ''
  let savedRevision = 0
  let savePhase: SavePhase = 'idle'
  let saveMessage = ''
  let pageError = ''
  let loadingInbox = true
  let loadingWorkspace = false
  let submitting = false
  let notificationState: NotificationState = 'checking'
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  let inboxTimer: ReturnType<typeof setInterval> | undefined
  let activeSave: Promise<boolean> | null = null
  const notificationTracker = new InboxNotificationTracker()

  $: dirty = workspace !== null && draftBody !== savedBody
  $: canSubmit =
    workspace !== null &&
    workspace.request.status !== 'completed' &&
    workspace.request.status !== 'cancelled' &&
    draftBody.trim().length > 0 &&
    !submitting

  onMount(() => {
    void initialize()
    void refreshNotificationPermission()
    inboxTimer = setInterval(() => void refreshInbox(), 5_000)
    return () => {
      if (saveTimer) clearTimeout(saveTimer)
      if (inboxTimer) clearInterval(inboxTimer)
    }
  })

  async function initialize() {
    pageError = ''
    loadingInbox = true
    try {
      const [nextHealth, nextEndpoint, nextInbox] = await Promise.all([
        invoke<HealthSnapshot>('get_health'),
        invoke<string>('get_mcp_endpoint'),
        invoke<FeedbackRequestSummary[]>('list_feedback_inbox'),
      ])
      health = nextHealth
      endpoint = nextEndpoint
      applyInboxSnapshot(nextInbox)
      if (nextInbox.length > 0) {
        await openRequest(nextInbox[0].request_id, false)
      }
    } catch (cause) {
      pageError = messageFrom(cause)
    } finally {
      loadingInbox = false
    }
  }

  async function refreshInbox() {
    try {
      const nextInbox = await invoke<FeedbackRequestSummary[]>('list_feedback_inbox')
      applyInboxSnapshot(nextInbox)
    } catch (cause) {
      pageError = messageFrom(cause)
    }
  }

  function applyInboxSnapshot(nextInbox: FeedbackRequestSummary[]) {
    const arrivals = notificationTracker.observe(nextInbox)
    inbox = nextInbox
    if (arrivals.length > 0 && notificationState === 'enabled') {
      sendNotification({
        title: 'RambleDesk',
        body:
          arrivals.length === 1
            ? '新的体验反馈请求已到达。打开工作台查看。'
            : `${arrivals.length} 个新的体验反馈请求已到达。打开工作台查看。`,
      })
    }
  }

  async function refreshNotificationPermission() {
    try {
      notificationState = (await isPermissionGranted()) ? 'enabled' : 'disabled'
    } catch {
      notificationState = 'unavailable'
    }
  }

  async function enableNotifications() {
    if (notificationState !== 'disabled') return
    notificationState = 'checking'
    try {
      const permission = await requestPermission()
      notificationState = permission === 'granted' ? 'enabled' : 'disabled'
    } catch {
      notificationState = 'unavailable'
    }
  }

  async function openRequest(requestId: string, saveCurrent = true) {
    if (workspace?.request.request_id === requestId) return
    if (saveCurrent && !(await saveDraftNow())) return

    loadingWorkspace = true
    pageError = ''
    completedResult = null
    try {
      const next = await invoke<FeedbackWorkspaceView>('get_feedback_workspace', {
        requestId,
      })
      workspace = next
      draftBody = next.draft.body_markdown
      savedBody = next.draft.body_markdown
      savedRevision = next.draft.saved_revision
      savePhase = next.draft.updated_at ? 'saved' : 'idle'
      saveMessage = ''
    } catch (cause) {
      pageError = messageFrom(cause)
    } finally {
      loadingWorkspace = false
    }
  }

  function updateDraft(value: string) {
    draftBody = value
    savePhase = draftBody === savedBody ? 'saved' : 'unsaved'
    saveMessage = ''
    if (saveTimer) clearTimeout(saveTimer)
    saveTimer = setTimeout(() => void saveDraftNow(), 700)
  }

  async function saveDraftNow(): Promise<boolean> {
    if (saveTimer) {
      clearTimeout(saveTimer)
      saveTimer = undefined
    }
    if (!workspace || !dirty) return true
    if (activeSave) {
      await activeSave
      return dirty ? saveDraftNow() : savePhase !== 'error'
    }

    const requestId = workspace.request.request_id
    const bodyToSave = draftBody
    const revisionToSave = savedRevision
    savePhase = 'saving'
    saveMessage = ''

    activeSave = (async () => {
      try {
        const input: SaveDraftInput = {
          request_id: requestId,
          body_markdown: bodyToSave,
          expected_revision: revisionToSave,
        }
        const saved = await invoke<DraftView>('save_feedback_draft', { input })
        if (workspace?.request.request_id === requestId) {
          savedBody = bodyToSave
          savedRevision = saved.saved_revision
          workspace = { ...workspace, draft: saved }
          savePhase = draftBody === bodyToSave ? 'saved' : 'unsaved'
        }
        return true
      } catch (cause) {
        savePhase = 'error'
        saveMessage = messageFrom(cause)
        return false
      }
    })()

    const succeeded = await activeSave
    activeSave = null
    if (succeeded && workspace?.request.request_id === requestId && draftBody !== savedBody) {
      return saveDraftNow()
    }
    return succeeded
  }

  async function reloadWorkspace() {
    const requestId = workspace?.request.request_id
    if (!requestId) return
    if (dirty && !(await saveDraftNow())) return
    workspace = null
    await openRequest(requestId, false)
  }

  async function submitFeedback() {
    if (!workspace || !canSubmit) return
    if (!(await saveDraftNow())) return

    submitting = true
    pageError = ''
    try {
      const input: SubmitFeedbackInput = {
        request_id: workspace.request.request_id,
        expected_revision: savedRevision,
      }
      const result = await invoke<FeedbackRequestView>('submit_feedback', { input })
      completedResult = result
      workspace = {
        ...workspace,
        request: {
          ...workspace.request,
          status: result.status,
          updated_at: result.updated_at,
        },
      }
      savePhase = 'saved'
      await refreshInbox()
    } catch (cause) {
      pageError = messageFrom(cause)
    } finally {
      submitting = false
    }
  }

  function formatTime(value: string | null | undefined): string {
    if (!value) return '尚未保存'
    const date = new Date(value)
    return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN')
  }

  function messageFrom(cause: unknown): string {
    if (cause instanceof Error) return cause.message
    if (
      cause &&
      typeof cause === 'object' &&
      'message' in cause &&
      typeof (cause as CommandError).message === 'string'
    ) {
      return (cause as CommandError).message
    }
    return String(cause)
  }
</script>

<svelte:head>
  <title>RambleDesk · Feedback Inbox</title>
</svelte:head>

<main class="shell">
  <header class="topbar">
    <div class="brand">
      <span class="brand-mark">R</span>
      <div>
        <strong>RambleDesk</strong>
        <small>体验反馈工作台</small>
      </div>
    </div>
    <div class="topbar-actions">
      <button
        class:enabled={notificationState === 'enabled'}
        class="notification-button"
        disabled={notificationState !== 'disabled'}
        onclick={enableNotifications}
        title="新请求通知不会包含项目或反馈内容"
      >
        {notificationLabel(notificationState)}
      </button>
      <div class="runtime" title={endpoint}>
        <span class:online={health?.status === 'ready'}></span>
        {health?.status === 'ready' ? 'MCP 在线' : '正在连接'}
      </div>
    </div>
  </header>

  <div class="workbench">
    <aside class="inbox-panel">
      <div class="panel-heading">
        <div>
          <p class="eyebrow">INBOX</p>
          <h1>待反馈</h1>
        </div>
        <button class="icon-button" aria-label="刷新待反馈请求" onclick={refreshInbox}>↻</button>
      </div>

      {#if loadingInbox}
        <p class="empty-state">正在读取持久请求…</p>
      {:else if inbox.length === 0}
        <div class="empty-state">
          <strong>当前没有待处理请求</strong>
          <span>保持工作台开启，Agent 的新请求会出现在这里。</span>
        </div>
      {:else}
        <nav aria-label="待反馈请求">
          {#each inbox as request (request.request_id)}
            <button
              class:active={workspace?.request.request_id === request.request_id}
              class="request-card"
              onclick={() => openRequest(request.request_id)}
            >
              <span class="request-meta">
                <b>{request.project_name}</b>
                <em>{requestStatusLabel(request.status)}</em>
              </span>
              <strong>{request.what_happened}</strong>
              <small>{request.agent} · {formatTime(request.updated_at)}</small>
            </button>
          {/each}
        </nav>
      {/if}

      <div class="connection-card">
        <span>Local MCP</span>
        <code>{endpoint}</code>
      </div>
    </aside>

    <section class="workspace-panel">
      {#if loadingWorkspace}
        <div class="workspace-placeholder">正在打开反馈工作区…</div>
      {:else if workspace}
        <header class="workspace-heading">
          <div>
            <div class="workspace-meta">
              <span>{workspace.request.project_name}</span>
              <span>{workspace.request.agent}</span>
              <span>{requestStatusLabel(workspace.request.status)}</span>
            </div>
            <h2>{workspace.request.what_happened}</h2>
            <p>Session · {workspace.request.session_id}</p>
          </div>
          <button class="secondary-button" onclick={reloadWorkspace}>重新载入</button>
        </header>

        <div class="task-sheet">
          <section>
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

          {#if workspace.context_refs.length > 0}
            <section class="context">
              <p class="eyebrow">CONTEXT</p>
              {#each workspace.context_refs as reference}
                <div>
                  <strong>{reference.label}</strong>
                  <code>{reference.uri}</code>
                </div>
              {/each}
            </section>
          {/if}
        </div>

        <section class="editor-section">
          <div class="editor-heading">
            <div>
              <p class="eyebrow">YOUR FEEDBACK</p>
              <h3>边体验，边记下来</h3>
            </div>
            <div class:failed={savePhase === 'error'} class="save-state" aria-live="polite">
              {#if savePhase === 'saving'}
                正在保存…
              {:else if savePhase === 'unsaved'}
                等待自动保存
              {:else if savePhase === 'error'}
                保存失败
              {:else}
                已保存 · revision {savedRevision}
              {/if}
            </div>
          </div>

          <textarea
            aria-label="Markdown 反馈正文"
            disabled={workspace.request.status === 'completed' || workspace.request.status === 'cancelled'}
            oninput={(event) => updateDraft(event.currentTarget.value)}
            placeholder="记录你看见了什么、哪里顺畅、哪里让你停顿。支持 Markdown。"
            value={draftBody}
          ></textarea>

          {#if saveMessage}
            <p class="inline-error">{saveMessage}。请重新载入后再试，当前文字仍保留在编辑器中。</p>
          {/if}

          {#if completedResult?.feedback}
            <div class="completion-card">
              <strong>反馈已提交</strong>
              <span>Agent 已可取得不可变 Feedback Package。</span>
              <code>{completedResult.feedback.directory_path}</code>
            </div>
          {/if}

          <footer class="editor-footer">
            <span>
              {draftBody.length.toLocaleString()} 字符 ·
              {formatTime(workspace.draft.updated_at)}
            </span>
            <button class="primary-button" disabled={!canSubmit} onclick={submitFeedback}>
              {submitting ? '正在发布…' : '提交反馈'}
            </button>
          </footer>
        </section>
      {:else}
        <div class="workspace-placeholder">
          <span class="placeholder-mark">↙</span>
          <strong>选择一个请求开始体验</strong>
          <p>任务清单和你的 Markdown 草稿都会持久保存在本机。</p>
        </div>
      {/if}

      {#if pageError}
        <div class="error-banner" role="alert">
          <strong>工作台暂时无法完成操作</strong>
          <span>{pageError}</span>
        </div>
      {/if}
    </section>
  </div>
</main>

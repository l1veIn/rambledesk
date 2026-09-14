// Scripted data for the interactive product mock, shaped like the real workbench objects:
// projects → sessions → requests, each request carrying a task brief and a feedback document.
// Nothing here talks to an agent; the surrounding copy says the whole window is demonstration data.

export type RequestStatus = "waiting" | "in_progress" | "completed" | "cancelled";
export type DeliveryState = "none" | "pending" | "sending" | "delivered";

export interface MockHost {
  id: string;
  label: string;
  /** Brand tone used by the rail's host mark. */
  tone: string;
}

export type DocumentBlock =
  | { kind: "text"; id: string; text: string }
  | { kind: "action"; id: string; label: string; text: string }
  | { kind: "speech"; id: string; text: string; pending: boolean }
  | { kind: "capture"; id: string; name: string; caption: string };

export interface MockRequest {
  id: string;
  title: string;
  status: RequestStatus;
  hostId: string;
  hostSessionId: string;
  sourceHint: string;
  updatedAt: string;
  createdAt: string;
  whatHappened: string;
  actions: { id: string; instruction: string }[];
  agentAttachments: { id: string; name: string; mediaType: string; sizeKiB: number }[];
  document: DocumentBlock[];
  /** Published requests with a refined body keep both versions. */
  cooked?: string;
  /** What one recorded segment writes into the document, and what Tidy makes of it. */
  speechLine: string;
  speechTidy: string;
  captureName: string;
  captureCaption: string;
  delivery: DeliveryState;
  packageFiles: { name: string; note: string }[];
}

export interface MockMessage {
  role: "human" | "agent";
  text: string;
  meta: string;
  tools?: string[];
}

export interface MockSession {
  id: string;
  title: string;
  hostId: string;
  pendingRequests: number;
  requestIds: string[];
  messages: MockMessage[];
}

export interface MockProject {
  key: string;
  kind: "project" | "external";
  name: string;
  cwd?: string;
  sessionIds: string[];
}

export interface MockUi {
  // Window chrome outside the product surface.
  exampleLabel: string;
  reset: string;
  footerNote: string;
  windowControls: string;
  minimizeWindow: string;
  maximizeWindow: string;
  closeWindow: string;
  // Host rail.
  newSession: string;
  projects: string;
  externalSessions: string;
  settings: string;
  search: string;
  allRequests: string;
  workspaceTabs: string;
  noSessions: string;
  collapseSidebar: string;
  expandSidebar: string;
  sessionActions: string;
  pinSession: string;
  archiveSession: string;
  // Request rail.
  requests: string;
  requestList: string;
  scope: string;
  filter: string;
  noRequests: string;
  noRequestsHint: string;
  loadMore: string;
  requestStatus: Record<RequestStatus, string>;
  // Workspace header.
  copyTaskBrief: string;
  untitledRequest: string;
  steps: string;
  acp: string;
  runtimeConnected: string;
  runtimeRunning: string;
  runtimeIdle: string;
  viewAgent: string;
  rambleFeedback: string;
  deliveryState: Record<DeliveryState, string>;
  deliveryDetail: Record<DeliveryState, string>;
  details: string;
  // Task brief.
  taskBrief: string;
  briefHeadline: string;
  whatHappened: string;
  actionsToExperience: string;
  reviewAgentAttachments: string;
  expand: string;
  collapse: string;
  fullscreenPreview: string;
  noTaskBrief: string;
  selectRequest: string;
  selectRequestHint: string;
  // Feedback document.
  feedbackDocument: string;
  documentHint: string;
  documentReadOnly: string;
  tidy: string;
  tidying: string;
  characters: string;
  markdown: string;
  saved: string;
  saving: string;
  waitingToAutosave: string;
  cooked: string;
  uncooked: string;
  restoreOriginal: string;
  cookedBanner: string;
  clickToEdit: string;
  actionGroupLabel: string;
  pendingSpeech: string;
  // Command rail.
  ramble: string;
  standby: string;
  recording: string;
  paused: string;
  startRecording: string;
  stopRecording: string;
  defaultMicrophone: string;
  segments: string;
  transcriptNote: string;
  addContext: string;
  capture: string;
  clipboard: string;
  files: string;
  clipboardNote: string;
  attachments: string;
  noAttachments: string;
  preview: string;
  delete: string;
  captureSettings: string;
  settingsItems: string[];
  settingsIssue: string;
  screenshotAlt: string;
  submitFeedback: string;
  publishing: string;
  cancelFeedback: string;
  feedbackPackage: string;
  published: string;
  openPackage: string;
  cancelled: string;
  cancelledNote: string;
  rambelle: string;
  rambelleStatus: string;
  rambelleLines: { idle: string; recording: string; published: string; cancelled: string };
  // Agent view and settings view.
  agentTab: string;
  settingsTab: string;
  agentPending: string;
  agentPendingHint: string;
  appearanceTitle: string;
  appearanceDescription: string;
  general: string;
  voice: string;
  notifications: string;
  about: string;
  theme: string;
  themeLight: string;
  themeDark: string;
  themeDetail: string;
  rowCompactRails: { label: string; detail: string };
  rowBrandBackground: { label: string; detail: string };
  rowMotion: { label: string; detail: string };
  settingsHint: string;
}

export interface MockContent {
  ui: MockUi;
  hosts: MockHost[];
  projects: MockProject[];
  sessions: MockSession[];
  requests: Record<string, MockRequest>;
}

const en: MockContent = {
  ui: {
    exampleLabel: "Interactive example · demonstration data",
    reset: "Restart the example",
    footerNote:
      "Interactive example: projects, sessions, requests, feedback and delivery states are demonstration data. Nothing here is connected to an agent.",
    windowControls: "Window controls",
    minimizeWindow: "Minimize window",
    maximizeWindow: "Maximize or restore window",
    closeWindow: "Close window",
    newSession: "New session",
    projects: "Projects",
    externalSessions: "External sessions",
    settings: "Settings",
    search: "Search sessions and projects",
    allRequests: "All requests",
    workspaceTabs: "Workspace",
    noSessions: "No matching sessions",
    collapseSidebar: "Collapse sidebar",
    expandSidebar: "Expand sidebar",
    sessionActions: "Session actions",
    pinSession: "Pin session",
    archiveSession: "Archive session",
    requests: "Requests",
    requestList: "Request list",
    scope: "Scope",
    filter: "Request filters",
    noRequests: "No requests in this scope",
    noRequestsHint: "New requests appear here by most recent update.",
    loadMore: "Load more",
    requestStatus: {
      waiting: "Waiting",
      in_progress: "In progress",
      completed: "Submitted",
      cancelled: "Cancelled",
    },
    copyTaskBrief: "Copy task brief",
    untitledRequest: "Untitled request",
    steps: "{count} steps",
    acp: "ACP",
    runtimeConnected: "Connected",
    runtimeRunning: "Agent is working",
    runtimeIdle: "Idle",
    viewAgent: "View Agent",
    rambleFeedback: "Ramble feedback",
    deliveryState: {
      none: "No feedback delivery yet",
      pending: "Waiting for the agent",
      sending: "Continuing the agent…",
      delivered: "Feedback delivered",
    },
    deliveryDetail: {
      none: "Submitting publishes an immutable package and continues this session.",
      pending: "Feedback is saved. It will continue this session when the agent can receive input.",
      sending: "The agent is continuing with this feedback.",
      delivered: "The continuation was sent to the agent.",
    },
    details: "Details",
    taskBrief: "Task brief",
    briefHeadline: "What happened · Actions to experience",
    whatHappened: "What happened",
    actionsToExperience: "Actions to experience",
    reviewAgentAttachments: "Review attachments from the agent",
    expand: "Expand",
    collapse: "Collapse",
    fullscreenPreview: "Fullscreen preview",
    noTaskBrief: "There is no task brief to preview.",
    selectRequest: "Select a request",
    selectRequestHint: "Choose a session and a request on the left to open its workspace.",
    feedbackDocument: "Feedback document",
    documentHint: "Record observations, problems, and suggestions.",
    documentReadOnly: "This request is closed. The document is read-only.",
    tidy: "Tidy",
    tidying: "Tidying…",
    characters: "{count} characters",
    markdown: "Markdown",
    saved: "Saved",
    saving: "Saving…",
    waitingToAutosave: "Waiting to autosave",
    cooked: "Cooked",
    uncooked: "Uncooked",
    restoreOriginal: "Restore original",
    cookedBanner: "Cooked; submitting will use this version.",
    clickToEdit: "Click to edit this block",
    actionGroupLabel: "@Action",
    pendingSpeech: "Pending speech",
    ramble: "Ramble",
    standby: "Standby",
    recording: "Recording",
    paused: "Paused",
    startRecording: "Start recording",
    stopRecording: "Stop recording",
    defaultMicrophone: "Default microphone",
    segments: "{count} segments",
    transcriptNote: "Audio is transcribed locally into the document.",
    addContext: "Add context",
    capture: "Capture",
    clipboard: "Clipboard",
    files: "Files",
    clipboardNote: "The clipboard is read once only when you click import.",
    attachments: "Attachments",
    noAttachments: "Captures and imported files are kept here.",
    preview: "Preview",
    delete: "Delete",
    captureSettings: "Settings",
    settingsItems: ["Account", "Notifications", "Appearance", "General", "About"],
    settingsIssue: "About is cut off at the bottom",
    screenshotAlt: "Illustrative capture: the “About” row is cut off at the bottom of the settings sidebar.",
    submitFeedback: "Submit feedback",
    publishing: "Publishing…",
    cancelFeedback: "Cancel feedback",
    feedbackPackage: "Feedback Package",
    published: "Published",
    openPackage: "Open feedback package",
    cancelled: "Cancelled",
    cancelledNote:
      "Feedback is cancelled. No continuation message is sent. The document can no longer be edited.",
    rambelle: "Rambelle",
    rambelleStatus: "Rambelle status",
    rambelleLines: {
      idle: "Commander, standing by. Start a Ramble when you want me.",
      recording: "Commander, I am recording this now.",
      published: "Package sealed, commander. I will not touch it again.",
      cancelled: "Commander, this request is closed. Nothing was sent.",
    },
    agentTab: "Agent",
    settingsTab: "Settings",
    agentPending: "Agent is working",
    agentPendingHint: "The agent has not asked you to check anything in this session yet.",
    appearanceTitle: "Appearance",
    appearanceDescription: "Preferences",
    general: "General",
    voice: "Voice",
    notifications: "Notifications",
    about: "About",
    theme: "Theme",
    themeLight: "Light",
    themeDark: "Dark",
    themeDetail: "Light paper surfaces with the ice-blue accent.",
    rowCompactRails: { label: "Compact rails", detail: "Keep the two navigation rails narrow." },
    rowBrandBackground: { label: "Brand background", detail: "Show the Rambelle pattern behind the workspace." },
    rowMotion: { label: "Motion", detail: "Play workspace transitions." },
    settingsHint: "Demonstration only: switches move, nothing is stored.",
  },
  hosts: [
    { id: "claude", label: "Claude Code", tone: "#c2703f" },
    { id: "codex", label: "Codex CLI", tone: "#3f7f6f" },
    { id: "gemini", label: "Gemini CLI", tone: "#4a72b8" },
    { id: "grok", label: "Grok", tone: "#5b5f7a" },
  ],
  projects: [
    {
      key: "rambledesk",
      kind: "project",
      name: "rambledesk",
      cwd: "C:\\Users\\A\\Desktop\\rambledesk",
      sessionIds: ["settings-narrow", "package-export", "first-run"],
    },
    {
      key: "kotone",
      kind: "project",
      name: "kotone",
      cwd: "C:\\Users\\A\\Desktop\\kotone",
      sessionIds: ["kotone-review"],
    },
    {
      key: "external",
      kind: "external",
      name: "External sessions",
      sessionIds: ["delivery-fix"],
    },
  ],
  sessions: [
    {
      id: "settings-narrow",
      title: "Settings · narrow layout",
      hostId: "claude",
      pendingRequests: 1,
      requestIds: ["rd-1042", "rd-1037"],
      messages: [
        { role: "human", text: "Make the settings page work in a narrow window.", meta: "You · 14:02" },
        {
          role: "agent",
          text: "Switched the settings sidebar to a fixed column and moved the list into its own scroll container. The horizontal scrollbar under the list is gone.",
          meta: "Claude Code · 14:07",
          tools: ["Edit src/lib/settings/SettingsSidebar.svelte", "pnpm -C apps/desktop check"],
        },
        {
          role: "agent",
          text: "The change is on disk. Before I continue, I need you to look at the narrow layout yourself.",
          meta: "Claude Code · 14:08",
        },
      ],
    },
    {
      id: "package-export",
      title: "Feedback package · keep the original",
      hostId: "codex",
      pendingRequests: 0,
      requestIds: ["rd-1041"],
      messages: [
        { role: "human", text: "Export the feedback package, I want to archive it.", meta: "You · 11:20" },
        {
          role: "agent",
          text: "Added a package export: manifest, Markdown and attachments are written to the folder you pick.",
          meta: "Codex CLI · 11:34",
          tools: ["cargo test -p rambledesk-core", "Edit crates/rambledesk-core/src/package.rs"],
        },
        {
          role: "agent",
          text: "Feedback received and read back into this session. Anything else you want in the export?",
          meta: "Codex CLI · 12:02",
        },
      ],
    },
    {
      id: "first-run",
      title: "First-run onboarding",
      hostId: "gemini",
      pendingRequests: 0,
      requestIds: [],
      messages: [
        { role: "human", text: "Add a first-run guide that helps a new user connect an agent.", meta: "You · 09:41" },
        {
          role: "agent",
          text: "Reading the onboarding flow and the settings entry points before I touch anything.",
          meta: "Gemini CLI · 09:43",
          tools: ["Read src/lib/onboarding/*", 'Grep "Web Access"'],
        },
        { role: "agent", text: "Still working through it. Nothing for you to check yet.", meta: "Gemini CLI · 09:45" },
      ],
    },
    {
      id: "kotone-review",
      title: "Review the uncommitted changes",
      hostId: "claude",
      pendingRequests: 1,
      requestIds: ["rd-1040"],
      messages: [
        { role: "human", text: "Review the uncommitted changes before I commit.", meta: "You · 16:12" },
        {
          role: "agent",
          text: "Twelve files changed in the app shell and the speech plugin. I left the diff summary in the brief.",
          meta: "Claude Code · 16:20",
          tools: ["git diff --stat", "Read src/speech/voiceSession.ts"],
        },
      ],
    },
    {
      id: "delivery-fix",
      title: "Delivery says “sent”, 0.4.0 is rebuilding",
      hostId: "grok",
      pendingRequests: 0,
      requestIds: ["rd-1038"],
      messages: [
        {
          role: "human",
          text: "delivered should mean the continuation message went into the session, not that the turn ended.",
          meta: "You · 17:40",
        },
        {
          role: "agent",
          text: "Adjusted: delivered now means the continuation message was written into the session. Closing the app, locking the screen or opening a new ACP session no longer marks sent feedback as unknown.",
          meta: "Grok · 18:05",
          tools: ["Edit crates/rambledesk-hosts/src/delivery.rs", "cargo test -p rambledesk-hosts"],
        },
        { role: "agent", text: "Commit bb8282. Tag v0.4.0 has been moved to that commit.", meta: "Grok · 18:12" },
      ],
    },
  ],
  requests: {
    "rd-1042": {
      id: "rd-1042",
      title: "Settings · narrow layout",
      status: "waiting",
      hostId: "claude",
      hostSessionId: "01a08e1c-4d21-7f60-9c33-8a1d5e77b204",
      sourceHint: "C:\\Users\\A\\Desktop\\rambledesk",
      createdAt: "9/11/2026, 2:08:41 PM",
      updatedAt: "9/11/2026, 2:08:41 PM",
      whatHappened:
        "The settings sidebar is now a fixed column and the list scrolls inside its own container. The horizontal scrollbar under the list is gone.",
      actions: [
        {
          id: "a1",
          instruction:
            "Open Settings → Appearance, narrow the window below 720px tall and check whether the “About” row is fully visible.",
        },
        {
          id: "a2",
          instruction: "Scroll the sidebar to the bottom and confirm no horizontal scrollbar appears.",
        },
        {
          id: "a3",
          instruction:
            "Switch between English and Chinese and check that no sidebar label wraps onto a second line.",
        },
      ],
      agentAttachments: [
        { id: "att-1", name: "settings-before.png", mediaType: "image/png", sizeKiB: 312 },
      ],
      document: [
        {
          kind: "text",
          id: "t1",
          text: "At about 720 tall the “About” row is still cut off — I can only see half of it. The horizontal scrollbar is gone, that part is fine.",
        },
        { kind: "capture", id: "c1", name: "settings-sidebar.png", caption: "Settings sidebar" },
      ],
      speechLine: "One more thing: when the window gets narrower, the description text on the right wraps onto two lines.",
      speechTidy: "Below that width, the description text on the right wraps onto two lines.",
      captureName: "settings-sidebar.png",
      captureCaption: "Settings sidebar",
      delivery: "none",
      packageFiles: [
        { name: "uncooked.md", note: "Original words, kept as written" },
        { name: "feedback.md", note: "Body the agent reads" },
      ],
    },
    "rd-1037": {
      id: "rd-1037",
      title: "Horizontal scrollbar in the sidebar",
      status: "cancelled",
      hostId: "claude",
      hostSessionId: "01a08e1c-4d21-7f60-9c33-8a1d5e77b204",
      sourceHint: "C:\\Users\\A\\Desktop\\rambledesk",
      createdAt: "9/11/2026, 1:26:03 PM",
      updatedAt: "9/11/2026, 1:34:55 PM",
      whatHappened:
        "The previous pass widened the sidebar content, which pushed a horizontal scrollbar under the list.",
      actions: [
        { id: "a1", instruction: "Narrow the window to 1000px wide and look at the bottom of the sidebar." },
        { id: "a2", instruction: "Scroll the list to its end and note where it stops." },
      ],
      agentAttachments: [],
      document: [
        {
          kind: "text",
          id: "t1",
          text: "The scrollbar is already gone after the last change, so this request is not needed any more.",
        },
      ],
      speechLine: "",
      speechTidy: "",
      captureName: "settings-sidebar.png",
      captureCaption: "Settings sidebar",
      delivery: "none",
      packageFiles: [],
    },
    "rd-1041": {
      id: "rd-1041",
      title: "Feedback package · keep the original",
      status: "completed",
      hostId: "codex",
      hostSessionId: "01a08df0-2b77-7c19-8e54-77c1aa90d318",
      sourceHint: "C:\\Users\\A\\Desktop\\rambledesk",
      createdAt: "9/11/2026, 11:41:22 AM",
      updatedAt: "9/11/2026, 12:02:10 PM",
      whatHappened:
        "Packing and exporting writes the manifest, Markdown and attachments into the folder you choose.",
      actions: [
        { id: "a1", instruction: "Export the latest package and open the folder." },
        { id: "a2", instruction: "Check that feedback.md matches what you submitted." },
      ],
      agentAttachments: [{ id: "att-1", name: "export-layout.md", mediaType: "text/markdown", sizeKiB: 2.4 }],
      document: [
        {
          kind: "text",
          id: "t1",
          text: "The export looks right, but the folder only has the refined version in it. I want my original words in there too, otherwise in a week I can’t check what I actually said.",
        },
      ],
      cooked:
        "The feedback package should contain uncooked.md alongside feedback.md. The current export only contains the refined version, so the original wording cannot be traced back later.",
      speechLine: "Can the package keep my original sentence next to the refined one?",
      speechTidy: "The package should keep the original wording next to the refined version.",
      captureName: "package-folder.png",
      captureCaption: "Export folder",
      delivery: "delivered",
      packageFiles: [
        { name: "uncooked.md", note: "Original words, stored with the package" },
        { name: "feedback.md", note: "Refined body the host reads by default" },
        { name: "settings-sidebar.png", note: "Attachment stored with the package" },
      ],
    },
    "rd-1040": {
      id: "rd-1040",
      title: "Uncommitted changes · app shell and speech",
      status: "waiting",
      hostId: "claude",
      hostSessionId: "01a08f22-9c10-7a44-b0f1-2f6c9d3ee511",
      sourceHint: "C:\\Users\\A\\Desktop\\kotone",
      createdAt: "9/11/2026, 4:20:38 PM",
      updatedAt: "9/11/2026, 4:20:38 PM",
      whatHappened:
        "Twelve files changed across the app shell and the speech plugin. Nothing is committed yet and the checks pass locally.",
      actions: [
        { id: "a1", instruction: "Open the diff summary and read the three largest files." },
        { id: "a2", instruction: "Confirm the speech plugin keeps its own credentials." },
        { id: "a3", instruction: "Check the shell changes against the narrow-window case." },
      ],
      agentAttachments: [{ id: "att-1", name: "diff-summary.md", mediaType: "text/markdown", sizeKiB: 6.1 }],
      document: [
        {
          kind: "text",
          id: "t1",
          text: "Read through the shell part: the rail resize looks fine. The speech plugin still reads its own API key, so that is unchanged.",
        },
      ],
      speechLine: "I did not get to the narrow-window case yet, I will look at it after the commit.",
      speechTidy: "The narrow-window case is not checked yet; it will be reviewed after the commit.",
      captureName: "diff-summary.png",
      captureCaption: "Diff summary",
      delivery: "none",
      packageFiles: [
        { name: "uncooked.md", note: "Original words, kept as written" },
        { name: "feedback.md", note: "Body the agent reads" },
      ],
    },
    "rd-1038": {
      id: "rd-1038",
      title: "Delivery says “sent”, 0.4.0 is rebuilding",
      status: "completed",
      hostId: "grok",
      hostSessionId: "01a08e0a-0c56-7f91-b9a3-dd2c2239eb47",
      sourceHint: "C:\\Users\\A\\Desktop\\rambledesk",
      createdAt: "9/11/2026, 5:51:03 PM",
      updatedAt: "9/11/2026, 6:12:47 PM",
      whatHappened:
        "Delivered no longer means this turn ended. It means the continuation message was written into the session and handed to the agent. Closing the app, locking the screen or reconnecting ACP no longer marks already-sent feedback as unknown.",
      actions: [
        { id: "a1", instruction: "Close the app while feedback is queued and reopen it." },
        { id: "a2", instruction: "Check that the request still reads “Feedback delivered” after the restart." },
      ],
      agentAttachments: [],
      document: [
        {
          kind: "text",
          id: "t1",
          text: "OK, but I looked again and something is off — CI seems to have gone stale.",
        },
      ],
      cooked:
        "The delivery semantics are correct. Separately: the CI pipeline appears to have gone stale and may need a rebuild.",
      speechLine: "",
      speechTidy: "",
      captureName: "delivery-state.png",
      captureCaption: "Delivery state",
      delivery: "delivered",
      packageFiles: [
        { name: "uncooked.md", note: "Original words, kept as written" },
        { name: "feedback.md", note: "Refined body the host read" },
      ],
    },
  },
};

const zh: MockContent = {
  ui: {
    exampleLabel: "交互示例 · 演示数据",
    reset: "重新开始演示",
    footerNote:
      "交互示例：项目、会话、请求、反馈与投递状态都是演示数据，未连接任何 Agent。",
    windowControls: "窗口控制",
    minimizeWindow: "最小化窗口",
    maximizeWindow: "最大化或还原窗口",
    closeWindow: "关闭窗口",
    newSession: "新建会话",
    projects: "项目",
    externalSessions: "外部会话",
    settings: "设置",
    search: "搜索会话和项目",
    allRequests: "全部请求",
    workspaceTabs: "工作区",
    noSessions: "没有匹配的会话",
    collapseSidebar: "收起侧栏",
    expandSidebar: "展开侧栏",
    sessionActions: "会话操作",
    pinSession: "固定会话",
    archiveSession: "归档会话",
    requests: "请求",
    requestList: "请求列表",
    scope: "范围",
    filter: "请求筛选",
    noRequests: "这个范围里还没有请求",
    noRequestsHint: "新请求会按最近更新出现在这里。",
    loadMore: "加载更多",
    requestStatus: {
      waiting: "等待开始",
      in_progress: "反馈中",
      completed: "已提交",
      cancelled: "已取消",
    },
    copyTaskBrief: "复制任务简报",
    untitledRequest: "未命名请求",
    steps: "{count} 个体验步骤",
    acp: "ACP",
    runtimeConnected: "已连接",
    runtimeRunning: "Agent 正在工作",
    runtimeIdle: "空闲",
    viewAgent: "查看 Agent",
    rambleFeedback: "Ramble 反馈",
    deliveryState: {
      none: "暂无反馈投递",
      pending: "等待 Agent",
      sending: "正在续接 Agent…",
      delivered: "反馈已送达",
    },
    deliveryDetail: {
      none: "提交后会发布一份不可变反馈包，并续接这个会话。",
      pending: "反馈已保存。Agent 可以接收输入时，会续接这个会话。",
      sending: "Agent 正在用这份反馈继续工作。",
      delivered: "续接消息已经写入会话，交给 Agent。",
    },
    details: "详情",
    taskBrief: "任务简报",
    briefHeadline: "发生了什么 · 需要体验",
    whatHappened: "发生了什么",
    actionsToExperience: "需要体验",
    reviewAgentAttachments: "审核 Agent 附带的材料",
    expand: "展开",
    collapse: "收起",
    fullscreenPreview: "放大预览",
    noTaskBrief: "没有可预览的任务简报。",
    selectRequest: "选择一个请求",
    selectRequestHint: "从左侧选一个会话和请求，打开它的工作区。",
    feedbackDocument: "反馈正文",
    documentHint: "记录观察、问题和建议。",
    documentReadOnly: "此请求已结束，正文只读。",
    tidy: "整理",
    tidying: "正在整理…",
    characters: "{count} 字符",
    markdown: "Markdown",
    saved: "已保存",
    saving: "正在保存…",
    waitingToAutosave: "等待自动保存",
    cooked: "整理稿",
    uncooked: "原稿",
    restoreOriginal: "恢复原稿",
    cookedBanner: "已整理；提交时会使用这一版。",
    clickToEdit: "点击编辑这一段",
    actionGroupLabel: "@Action",
    pendingSpeech: "待整理语音",
    ramble: "Ramble",
    standby: "待命",
    recording: "正在记录",
    paused: "已暂停",
    startRecording: "开始记录",
    stopRecording: "停止记录",
    defaultMicrophone: "默认麦克风",
    segments: "{count} 段",
    transcriptNote: "语音在本机转写成正文。",
    addContext: "添加上下文",
    capture: "截图",
    clipboard: "剪贴板",
    files: "文件",
    clipboardNote: "只有点击导入时才会读取一次剪贴板。",
    attachments: "附件",
    noAttachments: "截图和导入的文件会保存在这里。",
    preview: "预览",
    delete: "删除",
    captureSettings: "设置",
    settingsItems: ["账户", "通知", "外观", "通用", "关于"],
    settingsIssue: "「关于」在底部被截断",
    screenshotAlt: "示意截图：设置侧栏底部的「关于」被截断。",
    submitFeedback: "提交反馈",
    publishing: "正在发布…",
    cancelFeedback: "取消反馈",
    feedbackPackage: "反馈包",
    published: "已发布",
    openPackage: "打开反馈包",
    cancelled: "已取消",
    cancelledNote: "反馈已取消，不会发送续接消息；正文也不再可编辑。",
    rambelle: "Rambelle",
    rambelleStatus: "Rambelle 状态",
    rambelleLines: {
      idle: "指挥官，待命中。想开始 Ramble 就叫我。",
      recording: "指挥官，我正在记录。",
      published: "指挥官，这份反馈包已经封存，我不会再改。",
      cancelled: "指挥官，这条请求已经结束，什么都没有发出去。",
    },
    agentTab: "Agent",
    settingsTab: "设置",
    agentPending: "Agent 正在工作",
    agentPendingHint: "这个会话里，Agent 还没有请你确认什么。",
    appearanceTitle: "外观",
    appearanceDescription: "偏好设置",
    general: "通用",
    voice: "语音",
    notifications: "通知",
    about: "关于",
    theme: "主题",
    themeLight: "浅色",
    themeDark: "深色",
    themeDetail: "浅色纸面，配合冰蓝强调色。",
    rowCompactRails: { label: "紧凑侧栏", detail: "让两条导航栏保持窄一些。" },
    rowBrandBackground: { label: "品牌背景", detail: "在工作台后面显示 Rambelle 底纹。" },
    rowMotion: { label: "界面动效", detail: "播放工作区切换动画。" },
    settingsHint: "仅作演示：开关会动，但不会保存。",
  },
  hosts: [
    { id: "claude", label: "Claude Code", tone: "#c2703f" },
    { id: "codex", label: "Codex CLI", tone: "#3f7f6f" },
    { id: "gemini", label: "Gemini CLI", tone: "#4a72b8" },
    { id: "grok", label: "Grok", tone: "#5b5f7a" },
  ],
  projects: [
    {
      key: "rambledesk",
      kind: "project",
      name: "rambledesk",
      cwd: "C:\\Users\\A\\Desktop\\rambledesk",
      sessionIds: ["settings-narrow", "package-export", "first-run"],
    },
    {
      key: "kotone",
      kind: "project",
      name: "kotone",
      cwd: "C:\\Users\\A\\Desktop\\kotone",
      sessionIds: ["kotone-review"],
    },
    {
      key: "external",
      kind: "external",
      name: "外部会话",
      sessionIds: ["delivery-fix"],
    },
  ],
  sessions: [
    {
      id: "settings-narrow",
      title: "设置页 · 窄窗口布局",
      hostId: "claude",
      pendingRequests: 1,
      requestIds: ["rd-1042", "rd-1037"],
      messages: [
        { role: "human", text: "把设置页在窄窗口里的布局修一下。", meta: "你 · 14:02" },
        {
          role: "agent",
          text: "已把设置侧栏改成固定列，列表放进独立的滚动容器；列表下方的横向滚动条已移除。",
          meta: "Claude Code · 14:07",
          tools: ["Edit src/lib/settings/SettingsSidebar.svelte", "pnpm -C apps/desktop check"],
        },
        {
          role: "agent",
          text: "改动已经落在工作区。继续之前，需要你先看一眼窄窗口下的实际效果。",
          meta: "Claude Code · 14:08",
        },
      ],
    },
    {
      id: "package-export",
      title: "反馈包 · 保留原稿",
      hostId: "codex",
      pendingRequests: 0,
      requestIds: ["rd-1041"],
      messages: [
        { role: "human", text: "导出反馈包，我要归档。", meta: "你 · 11:20" },
        {
          role: "agent",
          text: "已加入打包导出：清单、Markdown 与附件会写入你选择的目录。",
          meta: "Codex CLI · 11:34",
          tools: ["cargo test -p rambledesk-core", "Edit crates/rambledesk-core/src/package.rs"],
        },
        { role: "agent", text: "反馈已读取并续接本次会话。导出里还想加什么？", meta: "Codex CLI · 12:02" },
      ],
    },
    {
      id: "first-run",
      title: "首次启动引导",
      hostId: "gemini",
      pendingRequests: 0,
      requestIds: [],
      messages: [
        { role: "human", text: "给新用户加一个接入 Agent 的引导。", meta: "你 · 09:41" },
        {
          role: "agent",
          text: "先读 onboarding 流程和设置入口，再动代码。",
          meta: "Gemini CLI · 09:43",
          tools: ["Read src/lib/onboarding/*", 'Grep "Web Access"'],
        },
        { role: "agent", text: "还在排查，暂时不需要你看什么。", meta: "Gemini CLI · 09:45" },
      ],
    },
    {
      id: "kotone-review",
      title: "审核一下未提交的代码",
      hostId: "claude",
      pendingRequests: 1,
      requestIds: ["rd-1040"],
      messages: [
        { role: "human", text: "提交之前先审一下未提交的改动。", meta: "你 · 16:12" },
        {
          role: "agent",
          text: "应用外壳和语音插件一共改了十二个文件，diff 摘要放在任务简报里了。",
          meta: "Claude Code · 16:20",
          tools: ["git diff --stat", "Read src/speech/voiceSession.ts"],
        },
      ],
    },
    {
      id: "delivery-fix",
      title: "投递成功改为「已发出」，0.4.0 正在重编",
      hostId: "grok",
      pendingRequests: 0,
      requestIds: ["rd-1038"],
      messages: [
        {
          role: "human",
          text: "delivered 应该是「续接消息已经写进会话」，不是「这一轮结束了」。",
          meta: "你 · 17:40",
        },
        {
          role: "agent",
          text: "已调整：delivered 现在表示续接消息已写入会话。关软件、锁屏或重开 ACP 会话，都不再把已发出的反馈标成投递未知。",
          meta: "Grok · 18:05",
          tools: ["Edit crates/rambledesk-hosts/src/delivery.rs", "cargo test -p rambledesk-hosts"],
        },
        { role: "agent", text: "提交 bb8282，标签 v0.4.0 已移到该提交。", meta: "Grok · 18:12" },
      ],
    },
  ],
  requests: {
    "rd-1042": {
      id: "rd-1042",
      title: "设置页 · 窄窗口布局",
      status: "waiting",
      hostId: "claude",
      hostSessionId: "01a08e1c-4d21-7f60-9c33-8a1d5e77b204",
      sourceHint: "C:\\Users\\A\\Desktop\\rambledesk",
      createdAt: "2026/9/11 14:08:41",
      updatedAt: "2026/9/11 14:08:41",
      whatHappened:
        "设置侧栏改为固定列，列表在自己的容器里滚动；列表下方的横向滚动条已移除。",
      actions: [
        {
          id: "a1",
          instruction: "打开 设置 → 外观，把窗口高度缩到 720px 以下，看「关于」一行是否完整可见。",
        },
        { id: "a2", instruction: "把侧栏滚到底，确认不再出现横向滚动条。" },
        { id: "a3", instruction: "切换中英文，确认侧栏文字没有折成两行。" },
      ],
      agentAttachments: [{ id: "att-1", name: "settings-before.png", mediaType: "image/png", sizeKiB: 312 }],
      document: [
        {
          kind: "text",
          id: "t1",
          text: "窗口大概 720 高的时候「关于」那一行还是被截掉，只露半行。横向滚动条是没有了，这块没问题。",
        },
        { kind: "capture", id: "c1", name: "settings-sidebar.png", caption: "设置侧栏" },
      ],
      speechLine: "还有一点：窗口再窄一点的时候，右边那段说明文字会挤成两行。",
      speechTidy: "窗口更窄时，右侧说明文字折成两行。",
      captureName: "settings-sidebar.png",
      captureCaption: "设置侧栏",
      delivery: "none",
      packageFiles: [
        { name: "uncooked.md", note: "原话，按写入时的样子保留" },
        { name: "feedback.md", note: "Agent 读取的正文" },
      ],
    },
    "rd-1037": {
      id: "rd-1037",
      title: "侧栏出现横向滚动条",
      status: "cancelled",
      hostId: "claude",
      hostSessionId: "01a08e1c-4d21-7f60-9c33-8a1d5e77b204",
      sourceHint: "C:\\Users\\A\\Desktop\\rambledesk",
      createdAt: "2026/9/11 13:26:03",
      updatedAt: "2026/9/11 13:34:55",
      whatHappened: "上一轮把侧栏内容撑宽，列表底部因此出现了横向滚动条。",
      actions: [
        { id: "a1", instruction: "把窗口缩到 1000px 宽，看侧栏底部。" },
        { id: "a2", instruction: "把列表滚到底，记下它停在哪里。" },
      ],
      agentAttachments: [],
      document: [
        { kind: "text", id: "t1", text: "上一轮改完之后滚动条已经没有了，这条请求不需要了。" },
      ],
      speechLine: "",
      speechTidy: "",
      captureName: "settings-sidebar.png",
      captureCaption: "设置侧栏",
      delivery: "none",
      packageFiles: [],
    },
    "rd-1041": {
      id: "rd-1041",
      title: "反馈包 · 保留原稿",
      status: "completed",
      hostId: "codex",
      hostSessionId: "01a08df0-2b77-7c19-8e54-77c1aa90d318",
      sourceHint: "C:\\Users\\A\\Desktop\\rambledesk",
      createdAt: "2026/9/11 11:41:22",
      updatedAt: "2026/9/11 12:02:10",
      whatHappened: "打包导出会把清单、Markdown 与附件写入你选择的目录。",
      actions: [
        { id: "a1", instruction: "导出最近一次反馈包，打开目录看一眼。" },
        { id: "a2", instruction: "确认 feedback.md 与提交内容一致。" },
      ],
      agentAttachments: [{ id: "att-1", name: "export-layout.md", mediaType: "text/markdown", sizeKiB: 2.4 }],
      document: [
        {
          kind: "text",
          id: "t1",
          text: "导出的样子没问题，不过目录里只有整理后的版本。我想把原话也放进去，不然过一周根本没法核对当时到底说了什么。",
        },
      ],
      cooked:
        "反馈包应同时包含 uncooked.md 与 feedback.md；当前导出只含有整理稿，之后无法回溯原始表达。",
      speechLine: "能不能把我原来那句话，和整理稿放在一起？",
      speechTidy: "反馈包应把原话与整理稿并列保存。",
      captureName: "package-folder.png",
      captureCaption: "导出目录",
      delivery: "delivered",
      packageFiles: [
        { name: "uncooked.md", note: "原话，随包一起保存" },
        { name: "feedback.md", note: "宿主默认读取的整理稿" },
        { name: "settings-sidebar.png", note: "随包保存的附件" },
      ],
    },
    "rd-1040": {
      id: "rd-1040",
      title: "未提交改动 · 应用外壳与语音",
      status: "waiting",
      hostId: "claude",
      hostSessionId: "01a08f22-9c10-7a44-b0f1-2f6c9d3ee511",
      sourceHint: "C:\\Users\\A\\Desktop\\kotone",
      createdAt: "2026/9/11 16:20:38",
      updatedAt: "2026/9/11 16:20:38",
      whatHappened: "应用外壳和语音插件一共改了十二个文件，还没有提交，本地检查都通过。",
      actions: [
        { id: "a1", instruction: "打开 diff 摘要，读一下最大的三个文件。" },
        { id: "a2", instruction: "确认语音插件仍然使用自己的凭据。" },
        { id: "a3", instruction: "对照窄窗口这一例检查外壳改动。" },
      ],
      agentAttachments: [{ id: "att-1", name: "diff-summary.md", mediaType: "text/markdown", sizeKiB: 6.1 }],
      document: [
        {
          kind: "text",
          id: "t1",
          text: "外壳这部分看过了：侧栏缩放没问题。语音插件仍然读自己的 API Key，这块没变。",
        },
      ],
      speechLine: "窄窗口那一种情况我还没看，提交完再看。",
      speechTidy: "窄窗口这一例尚未确认，计划在提交后复核。",
      captureName: "diff-summary.png",
      captureCaption: "diff 摘要",
      delivery: "none",
      packageFiles: [
        { name: "uncooked.md", note: "原话，按写入时的样子保留" },
        { name: "feedback.md", note: "Agent 读取的正文" },
      ],
    },
    "rd-1038": {
      id: "rd-1038",
      title: "投递成功改为「已发出」，0.4.0 正在重编",
      status: "completed",
      hostId: "grok",
      hostSessionId: "01a08e0a-0c56-7f91-b9a3-dd2c2239eb47",
      sourceHint: "C:\\Users\\A\\Desktop\\rambledesk",
      createdAt: "2026/9/11 17:51:03",
      updatedAt: "2026/9/11 18:12:47",
      whatHappened:
        "delivered 不再等于这一轮结束，而是续接消息已经写入会话交给 Agent。关软件、锁屏重新打开或重连 ACP，都不再把已发出的反馈标成投递未知。",
      actions: [
        { id: "a1", instruction: "在反馈排队时关掉软件再打开。" },
        { id: "a2", instruction: "确认重启后这条请求仍显示「反馈已送达」。" },
      ],
      agentAttachments: [],
      document: [
        { kind: "text", id: "t1", text: "OK，不过我又看了一下，好像出问题了 —— CI 好像已经失效了。" },
      ],
      cooked: "投递语义已符合预期。另外：CI 流水线似乎已经失效，可能需要重新构建。",
      speechLine: "",
      speechTidy: "",
      captureName: "delivery-state.png",
      captureCaption: "投递状态",
      delivery: "delivered",
      packageFiles: [
        { name: "uncooked.md", note: "原话，按写入时的样子保留" },
        { name: "feedback.md", note: "宿主读取的整理稿" },
      ],
    },
  },
};

export const productMock: Record<"en" | "zh-CN", MockContent> = { en, "zh-CN": zh };

export function mockContent(lang: string): MockContent {
  return lang === "zh-CN" ? productMock["zh-CN"] : productMock.en;
}

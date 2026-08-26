# Dogfooding rounds

## 2026-08-01 — floating console and blocking feedback wait

### Round 1: static and automated audit

- Scope: desktop title bar, settings entry points, floating Ramble console, Tauri icons, MCP feedback lifecycle, macOS compilation.
- Findings: the console used a wide text layout and had no reliable drag surface; it was centered instead of kept out of the user's way; feedback completion required repeated polling; the macOS audio stream owner was not `Send`; web preview mounted native Tauri listeners unconditionally.
- Fixes: replaced the console with a narrow vertical icon toolbar and explicit drag surface; positioned it at right-center with a 10 px logical inset; added `wait_feedback`; moved the native audio owner onto a dedicated thread; guarded native-only browser hooks; regenerated Tauri icon sizes from the current app icon.
- Evidence: `pnpm check`, `pnpm test`, `pnpm build:web`, `cargo fmt --all --check`, `cargo test --workspace`, and strict workspace clippy passed. MCP self-test and the Inspector smoke test list and invoke `wait_feedback`.

### Round 2: browser visual and interaction regression

- Scope: settings routing, language/theme controls, responsive shell, content pattern, floating console presentation.
- Findings: before native-hook guards, a plain browser preview entered a broken Svelte effect when Tauri metadata was unavailable. This prevented reliable interaction testing.
- Fixes: native lifecycle hooks now degrade cleanly outside Tauri while settings remain testable; opening settings no longer depends on the MCP configuration request succeeding.
- Evidence: General opens from the title-bar button; the Local MCP card opens the MCP section directly; Chinese/English and system/light/dark choices update the UI; 1320 px, 1180 px, and 980 px viewports have no horizontal overflow; the configured pattern layer is present; the console route renders an icon-only vertical toolbar.

### Remaining native acceptance

- macOS native interaction is blocked because the test machine is locked. Window dragging, native notification permission, microphone capture, and subjective seam/contrast inspection require an unlocked desktop.
- Windows native acceptance remains required for the requested 1320 px, 1180 px, and 980 px command-rail checks and platform-specific window controls.

## 2026-08-01 — native macOS acceptance

### Goal

- Exercise the current debug application bundle as a real operator and complete a feedback package while an MCP client is blocked in `wait_feedback`.

### Tester path

1. Opened General and MCP settings from their dedicated entry points, switched languages and all appearance modes, and toggled notifications off and back on.
2. Created durable local requests through the authenticated MCP endpoint and opened their full editor workspaces.
3. Started Ramble, inspected and dragged the vertical console, paused and resumed speech capture, invoked capture, and exited back to the main window.
4. Saved and submitted a rich-text feedback document while the Inspector client waited in `wait_feedback`.

### Findings

- The macOS speech path initially reported the expected missing-model error because model distribution is intentionally outside Git.
- The non-Windows region-capture availability error remained Chinese in an English UI.
- No layout overflow, pattern seam, command-rail clipping, notification-state failure, or native window-control regression was observed.

### Fixes shipped

- Installed the manifest-pinned X-ASR model into local application data after verifying the archive and every required file against the committed SHA-256 values.
- Localized the non-Windows region-capture availability message.

### Validation

- The native console rendered as a compact icon-only vertical toolbar and exposed hover/help labels for every action.
- Speech reached active capture, then paused, resumed, and exited cleanly.
- The completed workspace became read-only, showed the archived Rambelle portrait, and exposed its immutable Feedback Package.
- The blocked Inspector call returned once with `execution_mode: wait`, the full manifest, Markdown, and attachment paths after UI submission.
- Windows native acceptance was removed from this iteration at the user's direction because the active development environment is macOS; exact 1320 px, 1180 px, and 980 px browser regressions remain covered separately.

### Next focus

- Run the final automated regression, remove ignored obsolete design payloads from the final tree, and Squash merge the iteration.

## 2026-08-02 — adapter polish, Pi one-click install, and UI fixes

### Goal

- Verify the previous round's two feedback fixes (generic MCP collapsed by default, Pi one-click install) and collect the next set of operator findings.

### Fixes shipped

- Generic MCP adapter section is now a `<details>` collapsed by default; detection, selection, and install live inside the expanded body.
- New `install_pi_package` Tauri command resolves `packages/pi-rambledesk` from the source checkout or by walking up from the working/executable directory, locates the `pi` CLI (`RAMBLEDESK_PI_BIN` override), and runs `pi install` with a success/error surface in the UI.
- Settings backdrop now matches the window's 16 px bottom corner radius (the blur box previously showed square corners beyond the rounded window).
- Settings sidebar nav now keeps the chevron flush right with the count badge grouped beside it instead of squeezing the arrow.
- Removed the useless "Source package available" badge from the Pi native adapter card.
- Settings dialog now closes on Escape.
- Pi package (`packages/pi-rambledesk`) generates a stable per-call request id and retries transient connection failures with backoff, so the observed "request created but response lost" flake no longer produces duplicate requests; the server-side wait remains unbounded (verified: `wait_feedback` has no timeout).
- `docs/UI_SHADCN_MIGRATION.md` written as the first migration baseline; the next refactor supersedes its deferred, style-only scope with a full workbench information-architecture rebuild.

### Validation

- `cargo fmt --all --check`, strict workspace clippy, `cargo test --workspace --all-targets`, `pnpm check`, `pnpm test`, `pnpm build:web`, `pnpm contracts:check`, `pnpm mcp:self-test`, `pnpm mcp:inspector-smoke`, and `pnpm test:pi` all pass.
- New unit tests cover package-dir resolution (checkout root, upward walk, missing) and Pi retry idempotency.

### Next focus

- Operator will verify the waiting behavior of `request_ramble_feedback` (long wait without failure) and visually accept the modal corner fix, nav alignment, and badge removal.
- Execute the shadcn-svelte workbench rebuild per `docs/UI_SHADCN_MIGRATION.md`.

## 2026-08-03 — macOS 0.0.1 release acceptance

### Scope

- Exercise the universal macOS release bundle, the installed Pi package, the authenticated Local Server, and the complete capture-to-document path on macOS 26.3.1.
- Red-team the implementation against `docs/TERMINOLOGY.md`, including crate dependency directions and protocol/storage ownership.

### Native and Pi evidence

- A real Pi 0.83.0 RPC run created a Feedback Request through `request_ramble_feedback`, waited while the operator submitted feedback in RambleDesk, then created and received final approval in the same `host_session_id`. Pi observed `resolution: "approved"`, `status: "completed"`, and `terminate: true` before settling without another model turn.
- The authenticated generic MCP surface exposed exactly `request_feedback`, `get_feedback`, and `cancel_feedback`; the Pi-only continuation behavior remained outside generic MCP.
- The unsigned universal DMG was mounted and launched from its release bundle. Its executable contains both `arm64` and `x86_64`, its bundle version is `0.0.1`, and `hdiutil verify` accepted the image.
- macOS Screen & System Audio Recording permission was granted to the mounted release application, followed by the required quit-and-reopen cycle.
- The capture overlay opened from the Ramble console, accepted a free-form region, created an arrow annotation, selected and resized that annotation, finalized a 447 KiB PNG, and inserted it at the current document position. The editor preview visibly contained the resized arrow and the attachment count increased from zero to one.
- A second capture was cancelled with Escape; the console was restored and the attachment count remained one.
- DPI handling was exercised on a 3840×2160 display using a 1920×1080 logical mode. The selected logical region mapped to the expected captured-pixel dimensions, with no offset or clipping. The transparent custom chrome retained rounded window corners.
- The automation interface intentionally cannot invoke operating-system global shortcuts. `Ctrl + Shift + 1` registration, event routing, and visible shortcut affordance were therefore covered by startup/static validation; the button entry exercised the identical capture command and complete downstream path.

### Regression gates

- Frozen dependency installation, release-version consistency, terminology and dependency-boundary checks, generated-contract freshness, Rust formatting, the 500-line Rust module limit, strict workspace clippy, all Rust targets, Svelte diagnostics, frontend tests, the production web build, Pi tests, and the MCP Inspector smoke test passed.
- `cargo check --workspace --target x86_64-apple-darwin` passed in addition to the native Apple Silicon build, and the universal DMG completed with `CI=true` and `--no-sign`.

## 2026-08-13 — 以 ramble 形式开发：`/ramble` 斜杠命令

### 背景

- 开发流程想默认走 Ramble 循环：Reasonix 通过 RambleDesk 的 `request_feedback`
  发出持久化请求，等待人类在实际使用中提交反馈包，再用 `get_feedback` 读取。
- 实测发现只有人类明确说"用 MCP 工具发 Ramble"之后宿主才会走这条路，不会默认
  采用。反馈建议：把 MCP 工具与一个斜杠命令/skill 绑定，`/ramble` 直接驱动。

### 落地

- 新增 `.reasonix/commands/ramble.md`，注册为 Reasonix 项目斜杠命令 `/ramble`；
  `reasonix doctor capabilities --json` 确认 entry 为 winner、root 状态 ok。
- 命令内容规定了完整循环：`request_feedback`（`host_id=reasonix`、不传
  `request_id` 由服务器生成 UUID、详细反馈请求不设 `allow_finish`）→ 用交互
  确认工具等待（不轮询）→ `get_feedback` 读取反馈包 → 逐条实现 → 需要再确认时
  重复；人类明确放弃时用 `cancel_feedback`。
- `.gitignore` 为 `.reasonix/commands/` 增加例外：静态命令模板随仓库版本化，
  `.reasonix/` 下的运行时状态（desktop-topic json 等）仍被忽略。

### 验证

- `reasonix doctor capabilities --json`：`summary.commands = 1`，
  `commands.entries = [{ name: "ramble", status: "winner" }]`。
- 通过 RambleDesk MCP 实测一轮完整循环：创建请求
  `019ff943-e35b-7311-a56b-9e4aa3a70484` → ask 等待 → `get_feedback` 读到
  本反馈包（`feedback.md`），确认流程端到端可用。

## 2026-08-14 — dsh 适配器实测与打包环境安装修复

### 发现（dsh 链路首轮真实使用）

- `request_ramble_feedback` 首次调用即失败：
  `tool "request_ramble_feedback" returned invalid output`。请求在服务端
  已创建（`bcd66423-a255-41a8-9c7b-92aa43dd02a9`），但工具结果过不了 dsh
  注册表的 output schema 校验。根因：`feedbackToolResult` 返回 MCP 风格的
  `{ content, details }`，而 dsh 要求 execute() 返回值匹配声明的
  `{ text, details }`（`additionalProperties: false`），再由 `render`
  投影为模型可见内容。四个工具（request/resume/get/cancel）全部受影响。
- host 插件无热重载：修完源码并同步安装副本后，必须重启 dsh 才生效。
  重启后 `resume_ramble_feedback` 成功接回已完成的请求——resume 路径在
  真实重启场景下得到验证。

### 发现（打包后安装，用户实测反馈）

- 打包成 exe 后，Pi 原生适配器安装会弹出一个黑色控制台窗口，且首次安装
  约需十来秒；dsh 原生适配器直接报"找不到 package 下的 dsh 包"装不上。
- 根因：`tauri.conf.json` 的 `bundle.resources` 只打包了
  `pi-rambledesk`，没有打包 `dsh-rambledesk`（安装器代码早已支持打包资源
  路径，但资源根本没进包）；Pi 安装从 GUI 进程 spawn 控制台 shim 时未加
  `CREATE_NO_WINDOW`，Windows 会为它新建一个控制台窗口。

### 修复

- `packages/dsh-rambledesk/index.js`：`feedbackToolResult` 改为返回规范的
  `{ text, details }` 值；新增回归测试锁定值形状（无 `content` 键）。
- `apps/desktop/src-tauri/tauri.conf.json`：把 dsh 包的 `index.js`、
  `package.json`、`README.md` 打进 `bundle.resources`；通用 `ramble` skill
  由 `rambledesk-hosts::RAMBLE_SKILL_MD` 编译进桌面端并在安装时写入。
- `apps/desktop/src-tauri/src/pi_install.rs`：Windows 下 spawn `pi install`
  时设置 `CREATE_NO_WINDOW`，不再闪现黑框。
- Pi 安装完成提示补充"首次安装可能耗时十几秒"。
- 版本提升到 `0.0.2-rc.9`。

### 验证

- `packages/dsh-rambledesk`：20 个测试全过（含新增 output 值形状回归测试）。
- `rambledesk-desktop`：pi_install 7 个、dsh_install 11 个测试全过；
  `cargo fmt --check`、版本一致性检查通过。
- 本地无签名密钥，验收用 NSIS 包以
  `--config {"bundle":{"createUpdaterArtifacts":false}}` 构建（仅省略
  updater 签名产物，安装器本体一致）；正式发布仍走 CI 签名流程。

### 人工验收（rc.9，用户实测）

- Pi 原生适配器：不再弹出黑色控制台窗口（`CREATE_NO_WINDOW` 生效）；
  首次安装仍约十几秒。复测定位：二次安装同一路径（打包资源
  `%LOCALAPPDATA%\RambleDesk\pi-rambledesk` 与 dev 源码路径）均只需约
  0.8 秒——十几秒是全新安装时 pi 首次解析 peer 依赖（npm registry 网络
  往返）的冷缓存耗时，pi CLI 无 offline/跳过选项，无法从 RambleDesk 侧
  消除；界面提示"首次安装可能耗时十几秒"已覆盖该场景。
- dsh 原生适配器：安装约 1 秒完成，不再报"找不到 dsh 包"；重启 dsh 后
  `/ramble` 生效、插件可用。
- 无其他新问题。

## 2026-08-14（第二轮）— Cook 预览、dev 稳定性、大文件拆分、适配器 Logo

### dsh 插件

- 实测发现 `REQUEST_CONFLICT`：终态持久化走 `loadState()`，其 memory
  恢复逻辑在"刚清空的 pendingRequestId + 仍为 waiting 的持久化 phase"
  下把已完成请求 id 又填回 pending，下一个 request 复用旧 id 被服务端
  拒绝，直到进程重启。修复为直接读文件（无恢复副作用），新增回归测试
  （连续两次 request 必须 mint 新 id）；安装副本同步，重启 dsh 后生效。
- 该轮同时实测验证了 resume 路径：重启后 `resume_ramble_feedback`
  成功接回已完成请求。

### Cook 预览（方案 E）

- 按用户确认的设计实现：主按钮区保持单个"Cook 并提交"（一键即走），
  编辑器右上角新增低调"先看 Cook 结果"入口；整理稿写入编辑器（可改、
  可恢复原文），提交时复用未过期的整理稿（不二次调用模型）；关闭
  Cooking 丢弃整理稿缓存。预览前自动退出 Ramble（停麦克风，与提交一致）。
- 用户验收四项全绿（预览驱动、提示条、提交复用、恢复原文）。

### dev 稳定性（关键根因）

- 用户报告改代码后 dev 崩溃：`EBUSY watch '<file>.<pid>.<uuid>.tmpdir/<file>.tmp'`。
  根因是 dsh 文件后端的原子写入（临时文件+rename）被 vite 的 chokidar
  watcher 监视，Windows 上触发 EBUSY 崩溃；vite 一崩，WebView2 停在
  崩溃前的旧模块图，新代码（含修复）全部"不生效"，并出现图片
  ERR_CONNECTION_REFUSED。修复：vite `watch.ignored` 忽略
  `**/.*.tmpdir/**` 与 `**/*.tmp`（实测原子写不再崩溃）。
- 另一环境问题：vite 8 在 Node 21+ 只监听 IPv6（::1），WebView2 解析
  localhost 常走 127.0.0.1 → 资源请求被拒。vite 改为强制绑定 127.0.0.1。

### 大文件拆分

- `App.svelte` 1217 → 1000 行：抽出 feedbackText（纯函数）、
  feedbackDraftSession（结构化草稿与自动保存状态机）、cookingController（cook 流程）、
  publisherController（提交/发布）、publishedFeedback 归一化与类型。
- `ScreenshotOverlay.svelte` 1027 → 965 行：抽出 overlayGeometry（纯几何
  与布局函数）。
- 44 个前端测试全过（含新增 16 个纯函数单测）；无头浏览器回归 cook
  预览通过；用户全面回归无异常。
- 教训：大重构期间 HMR 会推送中间态代码导致用户页面崩溃
  （saveDraftNow before initialization），应避免在用户活跃测试时做
  跨文件重构，或提前提醒。

### 适配器 Logo 与续接

- dsh.svg 由通用终端图形换成 DeepSeek 鲸鱼标志（simple-icons DeepSeek，
  currentColor）；设置 → 适配器卡片同步使用（此前是通用 Bot 图标）。
- claude/cursor/gemini/openai/opencode/mcp 图标无 fill 属性（两种主题下
  都是黑色），统一加 `fill="currentColor"` 随主题变色。
- native-wait 续接策略只匹配 pi，dsh 终态后错误地落入 generic 手动续接
  弹 Resume Prompt；策略现同时匹配 pi 与 dsh（id "native"）。
- `present_resume_prompt` 不再 show/unminimize/set_focus 主窗口：
  通知+铃声足够，避免打断全屏游戏。
- 用户已验收鲸鱼 Logo；native 续接与不抢焦点待其在后续真实提交中确认。

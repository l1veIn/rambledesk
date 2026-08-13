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

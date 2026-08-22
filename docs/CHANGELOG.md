# 更新日志 / Changelog

每次发布前，请在本文件**顶部**为该版本新增一个条目，标题格式为 `## vX.Y.Z`。
发布管线（`.github/workflows/release.yml`）会在构建完成后自动把对应条目写入：

- GitHub Release 正文；
- 更新清单 `latest.json` 的 `notes` 字段（应用内"软件更新"弹窗显示的就是它）。

所以发布者只需要维护本文件，不需要手工复制文案。

编辑约定：

- 条目使用**纯文本**：更新弹窗以 `<pre>` 渲染，不解析 Markdown（避免 `**`、`-` 列表等符号原样显示）。
- **英文在前，中文摘要在后**，方便中英文用户。
- 未写条目的版本会自动回退到通用说明（管线不中断，但会打警告）。

---

## v0.3.1

What's new in RambleDesk 0.3.1

UI
- Onboarding adapters step: Pi, DSH, and generic MCP hosts are now three equal rows with theme-aware logos and bottom-aligned install buttons.
- The onboarding dialog no longer closes on outside clicks or Escape; only "Set up later" and the finish button close it.
- The Ramble console start button now uses the play triangle icon, matching the task brief preview.
- The request list header shows a dynamic count pill next to "Requests".

Reliability
- Startup failures (a database created by a newer app version, an unwritable log directory, and similar cases) now show a clear dialog and exit cleanly instead of a silent crash.
- Opening a database created by a newer app version reports an actionable message instead of a generic migration error.

中文摘要
- 界面：新手引导适配器步骤改为三行同款卡片，图标随浅色/深色主题变色，安装按钮底部对齐；引导弹窗不再因点击外部或 Esc 意外关闭；"开始记录"按钮图标改为三角形；请求列表标题旁新增数量胶囊。
- 可靠性：启动失败不再闪退，改为弹窗说明原因；旧版本打开新版本创建的数据库时会提示安装最新版。

Full changelog: https://github.com/l1veIn/rambledesk/compare/v0.3.0...v0.3.1

## v0.3.0

What's new in RambleDesk 0.3.0 (first 0.3.x stable release)

- Native adapters: Pi and DeepSeek Harness (DSH) adapters, plus generic MCP hosts (Antigravity support, SSE transport, subscriptions and multi-location skill injection).
- Workbench: archived session management, "last 24 hours" request filtering, local-path request attachments, and clickable links in task briefs and markdown previews.
- Updates: release notes are shown on launch after an update.
- Packaging: branded installers, the new web homepage, release-pipeline hardening, and stability fixes.

中文摘要
- 首个 0.3.x 稳定版：接入 Pi 与 DeepSeek Harness 原生适配器，以及通用 MCP 主机（支持 Antigravity、SSE 传输、订阅与多位置技能注入）。
- 工作台新增：会话归档管理、最近 24 小时过滤、本地路径附件、任务简报与预览中的可点击链接。
- 其他：启动时显示更新说明；安装包品牌化；发布管线加固与稳定性修复。

Full changelog: https://github.com/l1veIn/rambledesk/compare/v0.0.2...v0.3.0

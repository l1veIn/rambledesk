# RambleDesk Windows + macOS release checklist

RambleDesk 正式发行 Windows x86_64 安装器和 Apple Silicon macOS DMG。Windows 使用 Tauri
updater 签名但暂未接入 Authenticode；macOS 使用 ad-hoc 签名且暂未公证。Release Notes 和
README 必须明确对应的 SmartScreen / Gatekeeper 首次启动步骤。

本页是发布操作与验收清单，不表示当前版本已通过全部平台安装。实际构建和人工缺口见
[质量与待验清单](quality/README.md)；没有 Windows 或真实设备环境的项目保持待验。

## Automated gates

- `pnpm install --frozen-lockfile`
- `pnpm release:check v<version>`
- `cargo fmt --all --check`
- `pnpm check:rust-size`
- `pnpm check:frontend-size`
- `pnpm check:terminology`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `pnpm check`
- `pnpm test`
- `pnpm test:pi`
- `pnpm test:dsh`
- `cargo test --workspace --exclude rambledesk-desktop --locked`
- `cargo test -p rambledesk-desktop --locked --target-dir target/desktop`
- `pnpm build:web`
- `pnpm contracts:check`
- `pnpm test:workbench-tools`
- `cargo test -p rambledesk-core -p rambledesk-storage --features workbench-fixtures --locked`
- `pnpm test:workbench-extension`（合同检查后单独运行，期间不并发修改源码、生成合同或构建）
- `pnpm mcp:inspector-smoke`
- 稳定版：`pnpm -C apps/desktop tauri build --target x86_64-pc-windows-msvc --bundles nsis,msi`
- RC：`pnpm -C apps/desktop tauri build --target x86_64-pc-windows-msvc --bundles nsis`（WiX/MSI 不接受 `rc.1` 这类 SemVer 预发布标识）
- macOS：`pnpm -C apps/desktop tauri build --target aarch64-apple-darwin --bundles dmg`

不要把 `cargo build --release` 生成的裸二进制作为发行产物。它不会执行 Tauri 的
`beforeBuildCommand`，因此不能代表嵌入生产前端后的应用。

默认生产构建只发现八种正式工作台，旧 `single_choice` 保留兼容，开发评分 `rating_review` 不进入生产目录或视图 bundle。开发 feature 验收与扩展演练单独运行，不能据此把测试类型编入发行包。

## Signing, notarization and updater

- Windows 基础配置的 `bundle.createUpdaterArtifacts` 必须开启；macOS 平台配置必须覆盖为关闭。
- 公钥只写入 `apps/desktop/src-tauri/tauri.conf.json`。
- 私钥与密码只通过 GitHub Repository Secrets 提供：
  - `TAURI_SIGNING_PRIVATE_KEY`
  - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
- 私钥与密码必须分别做离线备份。丢失任何一项都会使已安装客户端无法验证后续更新。
- Release 必须包含 NSIS 安装器、对应 `.sig`、稳定版 MSI、Apple Silicon DMG、`latest.json`
  和覆盖 Windows/macOS 产物的 `SHA256SUMS.txt`。
- Tauri updater 签名不等于 Windows Authenticode 签名；在 Authenticode 接入前，Release
  Notes 必须明确说明可能出现 SmartScreen 提示。
- Tauri updater 签名也不等于 Apple Developer ID。macOS 当前平台配置使用 `"-"` 做
  ad-hoc 签名、关闭 updater artifact，`latest.json` 不得出现 darwin 平台。
- 在 Developer ID 和 notarization 接入前，Release Notes 必须说明 Gatekeeper 风险，并链接
  README 的右键打开、隐私与安全性“仍要打开”及 quarantine 处理步骤。

## macOS 分发合同

当前选择通过 GitHub Releases 提供未公证的 Apple Silicon DMG。Developer ID、公证、Intel/universal、
Homebrew Cask、Mac App Store 和 macOS 应用内更新均不在当前发行面内；后续启用须单独变更配置、
发布渠道与验收，不通过补一条文案暗示支持。

- [release.yml](../.github/workflows/release.yml) 的 macOS job 使用 `macos-15`，目标为
  `aarch64-apple-darwin`，通过 `tauri-action` 执行 `--bundles dmg`。当前不向该 job 传 Apple
  Developer ID 或公证凭据，也不将 `.app.tar.gz` 作为 macOS updater 来源。
- [tauri.macos.conf.json](../apps/desktop/src-tauri/tauri.macos.conf.json)保留
  `signingIdentity: "-"`、`createUpdaterArtifacts: false`，以及双语 DMG 背景与图标位置。
  平台覆盖只作用于 macOS，不关闭 Windows updater。
- [make-updater-json.mjs](../scripts/make-updater-json.mjs)只写 `windows-x86_64` /
  `windows-x86_64-nsis`；校验清单等待 Windows、macOS 两个构建完成后统一生成。
- 设置 → 关于在 macOS 提供 GitHub Releases 手动下载入口，不调用 Tauri updater；按真实平台显示名称。
- 首次安装文案保持同一顺序：浏览器下载并核对来源 → 挂载 DMG → 拖入 Applications → 推出映像 →
  右键打开或“隐私与安全性 → 仍要打开”。若仍显示“已损坏”，先核对 SHA-256；一致时才尝试下面的
  quarantine 恢复步骤，不能把所有损坏报告都直接归因为隔离属性。

```bash
xattr -dr com.apple.quarantine /Applications/RambleDesk.app
```

不要求 `sudo`，不使用无差别清除全部扩展属性的命令。README、Release Notes 与 DMG 的安装提示必须一致。

## Release candidate acceptance

RC 保留 SemVer 预发布后缀，在 GitHub 中保持 `prerelease=true`，发布时显式设置 `make_latest=false`。
不将 RC 临时改为普通 Release 来测试稳定更新。当前 tag 工作流创建 Draft 时已按预发布标签设置
`prerelease`；公开发布与 latest 选择仍需按本页核对。

1. 构建目标版本的 RC Draft，通过自动门禁并核对全部发行资产后，按明确授权发布为 Prerelease；随后完成以下安装验收并记录结果。
2. 在干净 Windows 用户环境中安装，确认首次启动进入新手引导。
3. 确认重复启动只聚焦已有窗口，不出现第二个本地服务器或数据库实例。
4. 验证新手引导中的 ACP 检测与连接、新建会话、首条消息、结构化反馈、保存草稿、提交和取消；ACP 集成流程不得触发外部适配器检测。
5. 验证 Agent 请求携带多份 Markdown 与图片附件，且 Markdown、Viewer.js 缩放均正常。
6. 验证设置 → 关于显示真实版本，检查更新失败时给出可理解的错误。
7. 发布后续 RC Prerelease，从前一个 RC 通过该版本安装器完成真实覆盖安装与重启。若验证自动
   updater，必须使用明确隔离测试更新端点的验收构建，记录其配置和签名结果；没有该环境则保留待验，
   不通过更改稳定 `/releases/latest` 来完成测试。
8. 在有进行中反馈或未保存草稿时，确认安装和重启按钮被禁用。
9. 验证覆盖安装、卸载、重新安装；卸载不得意外删除反馈库和反馈包。
10. 在干净 Apple Silicon Mac 上从浏览器下载 DMG，确认自定义安装背景、图标位置与双语
    Gatekeeper 提示均可见；拖入 Applications 后推出磁盘映像。
11. 依次验证右键 → 打开、系统设置 → 隐私与安全性 → 仍要打开，以及校验值一致时的
    `xattr -dr com.apple.quarantine /Applications/RambleDesk.app` 恢复路径。
12. 验证 macOS 新手引导、屏幕录制/麦克风权限、授权后重启、关闭窗口后 Dock 重新打开。
13. 确认 macOS 设置 → 关于只提供 GitHub Releases 手动更新，不调用 updater。
14. 核对 `SHA256SUMS.txt` 同时包含 Windows 安装器和 macOS DMG，并分别重新计算 SHA-256。
15. 按 [诊断包与问题反馈](RC_FIELD_FEEDBACK.md) 导出诊断 ZIP，核对新引导、ACP 连接、首响应、取消和会话管理的开始/结局与耗时；确认包中包含结构化事件、摘要和覆盖范围。
16. 将外部适配器作为保留用户原有 Agent 应用或 CLI 工作方式的轻量接入路径独立回归，RambleDesk 负责接收反馈请求并返回回复；只有显式进入设置页才执行其检测，不作为推荐的 ACP 集成流程的新用户验收前置条件。
17. 按 [数据兼容说明](DATA_COMPATIBILITY.md) 验证“0.3.3 → 新版 → 0.3.3 → 新版”：旧式反馈读写可用，ACP 与工作台数据保留。0.3.3 不理解新类型工作台的只读保护，不用旧版编辑或提交这些请求。另模拟未知数据库版本与读取超时，确认显示错误、恢复入口可用且不会无限加载。
18. 验证项目侧栏：同目录不同 Agent 的会话归入同一项目，同名不同路径分开；旧会话无目录时仍可打开。从项目中新建会话带入目录，全局新建要求选择目录；切换 Agent 保留文字和目录。检查置顶、归档、搜索及清除搜索，并在慢连接时快速切换到新建会话，确认旧加载结果不会覆盖当前页面。
19. 用 [工作台 Playground](../playground/workbenches/README.md) 的十个场景覆盖八种正式类型：自由反馈、逐项问答（含单题选择）、文稿审阅、网页评审、终端试用、视觉反馈（空白画布和图片背景）、差异评审及拖动排序，再完成汇总请求。检查共享输入的固定归属、集中整理、草稿恢复与结构化提交；文稿检查批注续写、收起和删除线，网页检查 bridge、视口及普通/全屏页签共用页面状态，终端检查停止/退出后重开、多轮记录、重连及提交前最终输出，视觉反馈检查绘制／撤销、缩放坐标、原图快照、PNG 生成与失败重试，差异评审检查 old/new 行范围及整块锚点、文件导航和只读回看，排序检查名称编辑、删除/恢复、键盘/拖动和最终结果。全屏仅验主动开启的类型，排序和普通模板不得出现入口；左侧会话导航保持可用。网页与终端首次引导及重看不得启动业务操作；简单问答和排序不增加引导。旧合同兼容单独验证，开发评分不计入正式数量；设备和平台未体验的项明确记为“未验证”。
20. 在桌面与 Web 验证目录浏览、起步卡片只填入不发送、关闭全部 tab 的空白页，以及内置会话指令的可见性；目录应属于 Backend Runtime 所在机器。

## Publishing

发行目标以本次授权的版本和根目录 `package.json` 的 `version` 为准，标签为 `v<version>`。
先运行 `pnpm release:check v<version>`，确认工作区清单、Cargo 元数据和 Tauri 配置一致；
发布的标签必须指向通过验证、包含本次交付的提交。

`CHANGELOG.md` 的 `Unreleased` 保存尚未发行的源码变化。确定本次版本后，将相关内容整理为该版本的 `## v<version>` 条目，保留已发布条目；`release-notes.mjs` 按标签抽取版本条目，不会把 `Unreleased` 自动当成发行说明。

Tag 工作流创建 Draft Release：RC 自动设置 `prerelease=true`，Windows 只构建 NSIS；
稳定版同时构建 NSIS/MSI。两者都必须等待 Apple Silicon DMG 和跨平台 `SHA256SUMS.txt`
生成完毕。Windows 安装器及其签名、`latest.json` 必须属于同一次构建；macOS 不生成
updater 签名，`latest.json` 只包含 Windows 平台。

发布动作需要明确授权，可通过 GitHub 界面或 CLI/API 执行。稳定版在自动门禁和安装验收通过后，
以 `draft=false, prerelease=false` 发布，并按发布授权设置 latest。RC 发布时使用
`draft=false, prerelease=true, make_latest=false`，标题及说明明确标注测试版本；公开测试包不代表
后续人工安装验收已经通过。

发布后逐项核对 Release API 的版本和状态、公开下载地址、`/releases/latest`，以及实际下载得到的
`/releases/latest/download/latest.json`。API 的 latest 与资产重定向都要检查，不能只凭其中一个
推断客户端更新目标。更新清单中的版本、平台、安装器 URL 和签名必须匹配对应 Release；重新下载
Windows/macOS 产物并计算 SHA-256，与该次构建的 `SHA256SUMS.txt` 对照。

同一 RC 标签的失败构建需要包含新修复时，不能只重跑旧工作流，因为旧 run 仍绑定原提交。
经明确授权重发尚未公开的 RC 时，先记录原标签指向与 Draft 资产，清除该 Draft 的旧资产或删除
该 Draft，再将标签更新到最终提交并触发完整双平台构建。不能保留旧 Windows 产物、只补新 macOS
产物；全部资产及更新清单、校验文件应从同一新提交重新生成。已公开版本优先使用新的版本号。

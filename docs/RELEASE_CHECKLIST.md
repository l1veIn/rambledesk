# RambleDesk Windows release checklist

RambleDesk 的首个正式发行渠道为 Windows x86_64。macOS 代码与 CI 兼容性可以继续维护，
但在 Developer ID 签名和 notarization 完成前，不向正式 GitHub Release 上传 macOS 产物。

## Automated gates

- `pnpm install --frozen-lockfile`
- `pnpm release:check v<version>`
- `cargo fmt --all --check`
- `pnpm check:rust-size`
- `pnpm check:terminology`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `pnpm check`
- `pnpm test`
- `pnpm test:pi`
- `cargo test --workspace --locked`
- `pnpm build:web`
- `pnpm contracts:check`
- `pnpm mcp:inspector-smoke`
- 稳定版：`pnpm -C apps/desktop tauri build --target x86_64-pc-windows-msvc --bundles nsis,msi`
- RC：`pnpm -C apps/desktop tauri build --target x86_64-pc-windows-msvc --bundles nsis`（WiX/MSI 不接受 `rc.1` 这类 SemVer 预发布标识）

不要把 `cargo build --release` 生成的裸二进制作为发行产物。它不会执行 Tauri 的
`beforeBuildCommand`，因此不能代表嵌入生产前端后的应用。

## Signing and updater

- `bundle.createUpdaterArtifacts` 必须开启。
- 公钥只写入 `apps/desktop/src-tauri/tauri.conf.json`。
- 私钥与密码只通过 GitHub Repository Secrets 提供：
  - `TAURI_SIGNING_PRIVATE_KEY`
  - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
- 私钥与密码必须分别做离线备份。丢失任何一项都会使已安装客户端无法验证后续更新。
- Release 必须包含 NSIS 安装器、对应 `.sig`、`latest.json` 和 `SHA256SUMS.txt`。
- Tauri updater 签名不等于 Windows Authenticode 签名；在 Authenticode 接入前，Release
  Notes 必须明确说明可能出现 SmartScreen 提示。

## Release candidate acceptance

GitHub 的 `/releases/latest` 不会选择 Draft 或 GitHub Prerelease。为了验证稳定更新端点，
RC 的版本号使用 SemVer 预发布后缀，但在 GitHub 中暂时按普通 Release 发布，并在标题与说明中
清楚标记为测试版本。稳定版发布后可以删除 RC Release 与标签。

1. 构建 `0.0.1-rc.1` Draft，通过自动门禁后手动发布为测试 Release。
2. 在干净 Windows 用户环境中安装，确认首次启动进入新手引导。
3. 确认重复启动只聚焦已有窗口，不出现第二个本地服务器或数据库实例。
4. 验证宿主适配器安装、重启宿主、创建反馈请求、保存草稿、提交和取消。
5. 验证 Agent 请求携带多份 Markdown 与图片附件，且 Markdown、Viewer.js 缩放均正常。
6. 验证设置 → 关于显示真实版本，检查更新失败时给出可理解的错误。
7. 发布 `0.0.1-rc.2` 测试 Release，从 `rc.1` 完成一次真实的检查、下载、安装与重启升级。
8. 在有进行中反馈或未保存草稿时，确认安装和重启按钮被禁用。
9. 验证覆盖安装、卸载、重新安装；卸载不得意外删除反馈库和反馈包。
10. 核对 `SHA256SUMS.txt`，下载 Release 安装器后重新计算一次 SHA-256。

## Publishing

Tag 工作流只创建 Draft Release。自动门禁和人工安装验收全部通过后，再在 GitHub 界面手动
发布正式版本。首个稳定版本使用 `v0.0.1`。

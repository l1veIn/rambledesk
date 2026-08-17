# macOS 按 Snow Shot 方式先发未公证 DMG

- 状态：Implemented
- 日期：2026-08-15

结论：现在跟 Snow Shot 同一档，不买 Apple 开发者账号，不公证。GitHub Release 上挂 Apple Silicon DMG；文档写清「已损坏」是隔离属性，不是包坏了。用户量起来再补 Developer ID。

Snow Shot 的实际做法（已核过他们的 DMG 和 CI）：ad-hoc 签名、官网 FAQ 教 `xattr`、`latest.json` 不含 darwin、Homebrew / App Store 都不做。RambleDesk 照抄这四条，并多守一条——应用内更新继续只服务 Windows，避免未公证包被 updater 装上去。

## 明确不做

- 不申请 Apple Developer Program，不导入证书，CI 不设 `APPLE_*` secrets
- 不把 `darwin-aarch64` 写进 `latest.json`
- 不做 Homebrew Cask、不做 Mac App Store
- 第一刀只出 `aarch64-apple-darwin`。Intel 用户极少，universal / x64 以后按需加

## 发版流水线

现有 tag → Draft Release 流程并行运行 macOS job。

改 [`.github/workflows/release.yml`](../.github/workflows/release.yml)：

- `build-windows` 保持现在的 NSIS / MSI 逻辑
- `build-macos` 使用 arm64 `macos-15` + `tauri-action`，参数 `--target aarch64-apple-darwin --bundles dmg`
- 不传任何 Apple 签名 / 公证环境变量
- 只上传 `.dmg`（不要把 `.app.tar.gz` 当更新源发出去）
- `checksums` job 等两个 build 都结束后重写一份包含 Windows 安装器 + macOS DMG 的 `SHA256SUMS.txt`
- Draft / Release Notes 改成同时说明：Windows 可能碰到 SmartScreen；macOS 未公证，首次打开需去掉隔离属性

macOS 构建配置用平台文件，避免动到 Windows updater。`apps/desktop/src-tauri/tauri.macos.conf.json` 包含：

- `bundle.targets`: `["app", "dmg"]`（CI 用 `--bundles dmg` 只上传 DMG）
- `bundle.createUpdaterArtifacts`: `false`
- `bundle.macOS.signingIdentity`: `"-"`（显式 ad-hoc，和 Snow Shot 实际产物同类）
- `bundle.macOS.dmg`: 使用双语自定义背景，展示拖入 Applications、首次右键打开、
  “隐私与安全性 → 仍要打开”和 README/quarantine 恢复提示

[`scripts/make-updater-json.mjs`](../scripts/make-updater-json.mjs) 保持只写 `windows-x86_64` / `windows-x86_64-nsis`。这是刻意的，不是漏了。

## 文档与安装说明

改 [`RELEASE_CHECKLIST.md`](RELEASE_CHECKLIST.md)：

- 标题和开篇从「Windows only，macOS 等公证」改成「Windows 正式渠道 + macOS 未公证 DMG」
- 稳定版 / RC 增加 `aarch64-apple-darwin` DMG 构建命令
- Signing 一节写清两套签名：Tauri updater 密钥（Windows 热更新）≠ Apple Developer ID（现在没有）
- Release 必备产物加上 Apple Silicon DMG
- 验收增加 macOS 路径：挂载 DMG → 拖进 Applications → 去掉 quarantine → 首次启动走完引导和权限；不要测应用内升级

改 [`README.md`](../README.md) / [`README.zh-CN.md`](../README.zh-CN.md)，加一小节「下载安装」：

- 指向 GitHub Releases
- macOS：拖到 Applications 后，若提示「已损坏，无法打开」，运行：

```bash
xattr -dr com.apple.quarantine /Applications/RambleDesk.app
```

- 写明这是未公证的开源构建，不是文件损坏
- 不要求 `sudo`；只删除 `com.apple.quarantine`，不无差别清除其他扩展属性

Release Notes 模板与 README 用同一段话，避免用户先去开 issue。

## 应用内更新：macOS 不要假装能升

[`apps/desktop/src/lib/AboutSettings.svelte`](../apps/desktop/src/lib/AboutSettings.svelte) 按平台显示 badge；macOS 不调用 updater，因为 `latest.json` 刻意没有 darwin 平台。

- badge 按运行平台显示 `Windows` / `macOS`
- macOS 上「软件更新」改成说明：请从 GitHub Releases 下新 DMG；按钮打开 Release 页，不调用 updater
- Windows 路径不动

## 验收

- tag 一次 RC，确认 Draft 里同时有 NSIS（RC 无 MSI）和 `aarch64` DMG，以及合并后的 `SHA256SUMS.txt`
- `latest.json` 仍然只有 Windows 平台
- 在干净 macOS 用户里：浏览器下载 DMG → 确认自定义安装提示 → 拖进 Applications →
  右键打开或“隐私与安全性 → 仍要打开”；若提示「已损坏」，核对 SHA-256 后执行
  `xattr -dr com.apple.quarantine /Applications/RambleDesk.app` → 新手引导和屏幕/麦克风权限能走完
- 设置 → 关于：macOS 不出现「检查更新失败」的 updater 报错
- Windows 安装、检查更新、覆盖安装回归一次

## 以后再正规化时

用户量起来后再单独做：Apple Developer Program、Developer ID、公证、`darwin-aarch64` 写入 `latest.json`、恢复 macOS 应用内更新。那是另一条 PR，不在这次范围。

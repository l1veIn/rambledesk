# RambleDesk

[English](README.md) | [简体中文](README.zh-CN.md)

**RambleDesk 是一个桌面应用，让你审阅 AI 编程助手的成果，用文字、语音和批注反馈，再继续原来的任务。**

Agent 说明做了什么、需要你体验或决定什么。你查看结果，直接留下意见。在 RambleDesk 内开始的会话中，提交反馈后会排队续接原对话。

[下载安装](https://github.com/l1veIn/rambledesk/releases/latest) · [开始使用](#开始使用) · [文档](docs/README.md)

- **对着材料提意见。** 圈出图片区域、评论改动行、填写单元格建议，不必写一大段话描述位置。
- **想到什么就说什么。** 支持文字、语音和附件；可选 AI 整理表达，原始反馈会保留。
- **接着原任务继续。** 草稿保留未完成的反馈；提交时，意见和附件一起交给 Agent。

## 看看怎么用

以下为 **0.5.0-rc.1 开发版**的界面截图，使用演示数据。这些新工作台**尚未包含在稳定版 0.4.0 中**。

**指出视觉问题：** 圈出需要调整的位置，直接说明改法。

![视觉反馈：圈选图片区域并填写意见](docs/screenshots/zh-CN/visual-feedback.webp)

**审阅代码改动：** 把意见留在对应的改动行旁边。

![差异评审：在代码改动行添加意见](docs/screenshots/zh-CN/diff-review.webp)

**提出表格建议：** 填写建议值，结合单元格说明原因。

![表格评审：填写单元格建议值和批注](docs/screenshots/zh-CN/table-review.webp)

这些操作记录反馈与修改建议，不会直接改动项目文件。

## 开始使用

从 [GitHub Releases](https://github.com/l1veIn/rambledesk/releases) 下载稳定版 **0.4.0**，支持 **Windows x64** 和 **macOS Apple Silicon**。要体验图中的新工作台，请按下方说明运行当前源码。

1. **连接 Agent。** 打开「设置 → Agents」，按提示安装支持的组件，或连接已有的 Claude Code、Codex CLI、Gemini CLI 等 Agent。登录与模型访问由 Agent 自身提供。
2. **给它一个任务。** 新建会话，选择 Agent 和项目目录，输入目标。
3. **体验并反馈。** 收到请求后，按体验步骤操作并提交意见。应用内会话会将反馈续接到原对话，工作台会显示送达状态。

文字反馈无需配置语音。需要说话时，在「设置 → 语音」下载本地转写模型并允许麦克风访问；AI 整理是可选功能，需另行配置模型服务。

Windows 安装包暂未做 Authenticode 签名，macOS 尚未公证，详见[安装指南](docs/INSTALLATION.zh-CN.md)。升级到 0.5.0 前请完整备份：0.4.0 无法打开升级后的数据库，回退方法见[数据兼容说明](docs/DATA_COMPATIBILITY.md)。

也可沿用原来的 Agent 应用或 CLI，在「设置 → 外部适配器」按[接入指南](docs/COMPATIBILITY.md)配置。续接方式因适配器而异，通用 MCP 需要手动回到 Agent 继续。

## 从源码运行

准备 Git、Node.js、pnpm、Rust 和对应平台的 Tauri 构建依赖后：

```bash
git clone https://github.com/l1veIn/rambledesk.git
cd rambledesk
pnpm install --frozen-lockfile
pnpm dev
```

更多说明：[开发环境](docs/DEVELOPMENT.md) · [Agent 配置与恢复](docs/ACP_MANAGED_SESSIONS.md) · [工作台 Playground](playground/workbenches/README.md) · [文档索引](docs/README.md)

## 致谢

- [Codeg](https://github.com/xintaofei/codeg)：ACP 接入、Agent 对话、设置与外观设计参考
- [Snow Shot](https://github.com/mg-chao/snow-shot)：截图能力
- [RepoChan](https://github.com/l1veIn/repochan-mono)：品牌与角色资产
- [Kotone](https://github.com/l1veIn/kotone)：本地语音转写的实现基础

## 许可证

[MIT](LICENSE)。改写代码及其他第三方组件保留各自许可证，详见[第三方声明](THIRD_PARTY_NOTICES.md)。

<p align="center">
  <img src="docs/social/rambelle-chibi-footer.webp" alt="Q 版 Rambelle 递交反馈档案包" width="400" />
</p>

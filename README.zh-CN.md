# RambleDesk

[English](README.md) | [简体中文](README.zh-CN.md)

**2026 年，Vibe Coding 里最昂贵的东西，是人的注意力。**

跟通用 Agent 聊天时，我们常常要做三件事：

1. 读完一段又长又散的模型自述。
2. 自己判断「它到底做完了什么、要我干什么」。
3. 再把想法写成一段像样的下一轮 prompt。

RambleDesk 把前两步变成一张**体验单**，把第三步改成**先说出来，再可选整理**。

Agent 必须先写清楚两件事：**刚才发生了什么**，以及**请你体验或确认什么**。你在桌面工作台里说话、截图、直接编辑，不用先写 prompt。提交后，原始反馈、可选整理稿和附件保存为不可变反馈包，交还 Agent 继续工作。把注意力留给体验和判断。

从 **0.4.0 候选版**起，RambleDesk 支持通过 ACP 连接 Claude Code、Codex CLI、Gemini CLI 等 Coding Agent。沿用你本地已经能正常工作的 Agent，按提示安装必要的连接组件，选择项目就能开始。

<div align="center">

<img src="https://github.com/l1veIn/rambledesk/releases/download/v0.3.2/rambledesk-demo-10s.gif" alt="RambleDesk 产品演示" width="960" />

<p><em>早期版本的反馈流程：Agent 请求体验 → 你说话、截图 → 提交反馈。</em></p>

</div>

## 快速开始

从 [GitHub Releases](https://github.com/l1veIn/rambledesk/releases) 下载 **0.4 候选版**，支持 Windows x64 和 macOS Apple Silicon。版本变化见[发布说明](docs/CHANGELOG.md)。

1. **连接 Agent。** 先确认它在本机能正常使用，再打开「设置 → Agents」，按提示完成连接。登录与模型访问由 Agent 自身提供。
2. **给它一个任务。** 新建会话，选择 Agent 和项目目录，输入目标。
3. **体验，再反馈。** 收到 Ramble 请求后，按体验单操作，用语音、截图或文字记录感受。提交后，RambleDesk 将反馈续接到原 Agent 会话；送达状态和恢复入口都在工作台里。

使用语音前，在「设置 → 语音」下载本地转写模型，并允许麦克风访问。需要模型帮你整理表达时，再到「设置 → 后处理」配置模型服务；整理默认关闭，原始反馈会保留。

<details>
<summary><strong>首次安装提示</strong></summary>

**Windows：**运行 `x64-setup.exe`。当前安装包未做 Authenticode 签名；若 SmartScreen 拦截，确认来源是本仓库后，选择「更多信息 → 仍要运行」。

**macOS：**打开 DMG，将 RambleDesk 拖入「应用程序」。当前版本采用 ad-hoc 签名，尚未公证。首次启动可右键选择「打开」；若仍被阻止，前往「系统设置 → 隐私与安全性」，在「安全性」区域点击 RambleDesk 对应的「仍要打开」，再确认「打开」。

若提示应用已损坏，先确认下载来源，再运行：

```bash
xattr -dr com.apple.quarantine /Applications/RambleDesk.app
```

</details>

## 沿用自己的 Agent 工作方式

推荐直接在 RambleDesk 中通过 **ACP** 开始对话。模型选项、权限和会话恢复能力取决于所连接的 Agent，配置与恢复方法见 [ACP 使用指南](docs/ACP_MANAGED_SESSIONS.md)。

也可以继续使用原来的 Agent 应用或 CLI，在「设置 → 外部适配器」接入反馈工作台。续接方式因适配器而异；通用 MCP 需要回到 Agent 继续。

## 从源码运行

准备 Node.js、pnpm、Rust 和对应平台的 Tauri 构建依赖后：

```bash
pnpm install --frozen-lockfile
pnpm dev
```

运行与检查方式见[开发指南](docs/DEVELOPMENT.md)，使用、接入与架构说明从[文档索引](docs/README.md)进入。

## 致谢

- [Codeg](https://github.com/xintaofei/codeg)：ACP 接入、Agent 对话、设置与外观设计参考
- [Snow Shot](https://github.com/mg-chao/snow-shot)：截图能力
- [RepoChan](https://github.com/l1veIn/repochan-mono)：品牌与角色资产
- [Kotone](https://github.com/l1veIn/kotone)：本地语音转写的实现基础

## 许可证

[MIT](LICENSE)。改写代码及其他第三方组件保留各自许可证，详见[第三方声明](THIRD_PARTY_NOTICES.md)。

<p align="center">
  <img src="docs/social/rambelle-chibi-footer.webp" alt="Q 版 Rambelle 递交反馈档案包" width="800" />
</p>

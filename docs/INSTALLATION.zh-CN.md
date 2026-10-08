# 安装 RambleDesk

[English](INSTALLATION.md) | [简体中文](INSTALLATION.zh-CN.md)

从 [GitHub Releases](https://github.com/l1veIn/rambledesk/releases/latest) 下载 Windows x64 安装包或 macOS Apple Silicon 磁盘映像。

升级前先阅读[数据兼容说明](DATA_COMPATIBILITY.md)。升级到 0.5.0 前请完整备份数据目录：0.4.0 无法直接打开由 0.5.0 升级后的数据库。

## Windows

运行下载的 `x64-setup.exe`，按安装程序提示操作。

当前安装包尚未做 Authenticode 签名。如果 SmartScreen 拦截，先确认文件来自本仓库，再选择「更多信息 → 仍要运行」。

## macOS

打开 DMG，将 RambleDesk 拖入「应用程序」。当前版本采用 ad-hoc 签名，尚未公证。

首次启动可右键点击 RambleDesk，选择「打开」。如果仍被阻止，前往「系统设置 → 隐私与安全性」，在「安全性」区域点击 RambleDesk 对应的「仍要打开」，再确认「打开」。

若系统提示应用已损坏，先确认下载来源，再移除该应用的隔离标记：

```bash
xattr -dr com.apple.quarantine /Applications/RambleDesk.app
```

## 开始第一个任务

1. 打开「设置 → Agents」，选择 Agent，按提示安装或连接，并完成 Agent 自己的登录或模型配置。
2. 新建会话，选择 Agent 和项目文件夹，输入任务。
3. 收到反馈请求后，体验结果并提交意见。在 RambleDesk 内开始的会话中，反馈会排队交回同一个 Agent 会话，送达状态在界面中可见。

文字反馈不需要配置语音或后处理模型。使用语音时，在「设置 → 语音」下载转写模型，并允许麦克风访问。可选的 AI 文字处理需要另外配置模型服务，原始反馈会保留。

连接与恢复细节见 [Agent 使用指南](ACP_MANAGED_SESSIONS.md)。如果想继续使用外部 Agent 应用或 CLI，见[外部适配器](COMPATIBILITY.md)；通用 MCP 需要回到 Agent 继续任务。

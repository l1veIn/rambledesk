# RambleDesk 产品文档

> 状态：v2 当前基线。
> 术语源：[TERMINOLOGY.md](TERMINOLOGY.md)。本文若与术语表冲突，以术语表为准。

## 一句话

RambleDesk 是本地人类反馈工作台：宿主智能体通过适配器请求人类真实使用和反馈，人类在桌面工作台中 ramble、截图、批注并提交，宿主智能体读取不可变反馈包后继续。

## 产品判断

- 编码智能体能写更多代码，但仍需要人类做真实判断、体验和取舍。
- 人类反馈不应被淹没在聊天上下文里；它需要请求、待办、草稿、附件、提交和不可变结果。
- RambleDesk 不内置智能体运行时，不内置 shell multiplexer，不持有源码 checkout 模型。
- 宿主通过适配器接入：Generic MCP 是通用路径，Pi package 是首个原生路径。
- 反馈正确性不依赖某次 tool call 或连接存活；请求与反馈包必须先落盘。

## 非目标

MVP 不做：

- 内置完整智能体运行时；
- 管理源码 checkout 或 workspace；
- 要求源码 checkout 路径；
- 云同步、账号体系、多人协作；
- 移动端完整 App；
- 独立常驻 MCP 网关；
- 用 CLI resume 探针伪装宿主原生适配器；
- 通用系统级听写工具；
- 多阶段智能体编排流水线（仅提供人类可选的单步 Feedback Cooking）。

## 核心对象

| 对象 | 含义 |
| --- | --- |
| 反馈请求 | 宿主智能体发给人类的一次体验/检查任务，用 `request_id` 标识。 |
| 反馈包 | 人类提交后生成的不可变证据，包含 markdown、manifest 和附件。 |
| 宿主 | 宿主智能体运行环境，例如 Pi、Claude Code、Codex、OpenCode、Reasonix。 |
| 宿主会话 | 宿主中的原对话、任务或运行上下文；同一会话可产生多次请求。 |
| 适配器 | 宿主接入 RambleDesk 的完整流程。 |
| 工作台 | 人类处理反馈请求的桌面 UI。 |
| Ramble | 工作台中的自由反馈采集模式，包含语音、文字和截图。 |
| Cooking | 提交前可选的大模型编辑步骤；把 Uncooked Feedback 整理为 Cooked Feedback，同时保留原稿。 |

## MVP 范围

| 模块 | 内容 |
| --- | --- |
| 桌面工作台 | Inbox、Request Workspace、Resume Prompt、Settings / Adapters、Tray。 |
| 通用 MCP 适配器 | MCP `request_feedback`、`get_feedback`、`cancel_feedback`；终态后手动 continuation。 |
| Pi 原生适配器 | `packages/pi-rambledesk`；通过本地 JSON API request/get/wait/cancel；Pi tool call 内等待。 |
| 本地服务 | loopback listener、auth、Host/Origin guard、`/api`、`/mcp` route mounting。 |
| 存储 | 反馈请求、草稿、附件 metadata、宿主会话关联、不可变反馈包。 |
| continuation | 通用 MCP 手动继续；Pi 无提交后继续；未来原生 continuation 预留。 |
| 通知 | 系统通知和工作台提示，均为 best-effort side effect。 |
| 设置 | 通用偏好、首次使用引导、Cooking 模型服务、语音模型，以及各适配器的安装结果与配置说明。 |

## 主流程

### 安装与待命

1. 安装并打开 RambleDesk；全新安装先进入新手引导。
2. 在引导第一步选择数据位置；若改动位置，保存并重启后才继续下载模型或产生反馈。
3. 可在引导中安装 Pi 原生适配器（同一 tool call 内自动继续），或按需配置通用 MCP 适配器（终态后手动继续）。
4. 工作台保持开启，可以托盘待命。

### 通用 MCP 适配器

1. 宿主智能体调用 `request_feedback`，携带 `host_id`、`host_session_id`、`what_happened`、`actions` 和可选 context hint。
2. RambleDesk 持久化反馈请求并通知人类。
3. 宿主智能体结束当前 turn。
4. 人类在工作台中使用、检查、截图、ramble，形成 Uncooked Feedback。
5. 若启用 Cooking，工作台调用人类配置的模型服务生成 Cooked Feedback；失败时保留原稿且不发布。
6. 人类提交，RambleDesk 发布同时包含 `feedback.md` 与 `uncooked.md` 的反馈包。
7. RambleDesk 显示手动 continuation 提示。
8. 人类回到宿主。
9. 宿主智能体调用 `get_feedback(request_id)` 读取 `feedback.md` 并继续。

通用 MCP 适配器不承诺自动恢复原宿主上下文。

### Pi 原生适配器

1. Pi 调用 `request_ramble_feedback`。
2. Pi package 调用 `/api/feedback/request` 创建请求。
3. Pi package 在同一 tool call 中调用 `/api/feedback/wait`。
4. 人类在工作台中提交或取消。
5. wait 返回终态反馈包。
6. Pi 在原 tool call 流程中继续。

Pi 原生适配器不需要提交后的 continuation。

### 异常与恢复

- 工作台未开启：适配器连接失败，宿主智能体可稍后复用同一 `request_id`。
- 连接中断：只结束 transport attempt，不取消反馈请求。
- 重复请求：相同 `request_id` + 相同不可变输入返回同一请求。
- 输入冲突：相同 `request_id` + 不同不可变输入返回 conflict。
- 已完成：返回原反馈包。
- 已取消：返回取消状态，不隐式重新打开。

## 桌面信息架构

```text
RambleDesk
├── Inbox
│   ├── 宿主 / 会话筛选栏
│   └── 当前范围内的 requests 列表
├── Request Workspace
│   ├── 任务说明
│   ├── actions 清单
│   ├── ramble 录音/转写
│   ├── 截图和附件
│   ├── Uncooked Feedback 草稿
│   ├── 可选 Cooking
│   └── Cook 并提交 / 直接提交 / 取消
├── Resume Prompt
│   └── 通用 MCP 手动继续提示
├── Settings / Adapters
│   ├── 通用 → 再次启用新手引导
│   ├── 通用 MCP 适配器
│   ├── Pi 原生适配器
│   ├── 通知
│   ├── 外观和语言
│   └── 语音/转写
└── Tray
    ├── 待处理角标
    └── 打开工作台 / 适配器设置 / 退出
```

UI 只在通用适配器设置内展示 MCP 配置。终态请求与待处理请求按更新时间出现在同一
requests 列表中，不按终态拆分独立分页。

## 请求字段原则

必需：

- `host_id`
- `host_session_id`
- `what_happened`
- `actions`

可选：

- `request_id`
- `title`
- `context_refs`
- `source_hint`

原则：

- 请求侧清晰、少发挥：`what_happened` + 可执行 `actions[]`。
- 回复侧自由：人类可以 ramble、截图、批注，最终产物是 markdown + attachments。
- `host_id` 与 `host_session_id` 只用于关联和 strategy 选择，不用于认证。
- RambleDesk 不要求源码 checkout 路径。

## 反馈包

默认写入 RambleDesk 应用数据目录：

```text
<local-data>/RambleDesk/feedback/<timestamp>-<request-id>/
  feedback.md       # 宿主默认读取的正式结果
  uncooked.md       # 人类原始反馈证据
  manifest.json
  attachments/
```

规则：

- 每次 completed 提交对应一份不可变反馈包。
- `uncooked.md` 始终保留；未启用 Cooking 时允许与 `feedback.md` 内容相同。
- manifest 记录 Cooking 模型标识与两份 Markdown 的 hash，但绝不记录 API Key。
- 宿主智能体继续前必须读取反馈包中的 `feedback.md`。
- 返回路径只保证同机、共享文件系统可见。
- 适配器提供的路径只能作为 context hint 或未来导出目标，不是协议前提。

## 成功指标

- 完整闭环次数：request → human feedback → package → 宿主智能体继续。
- 从通知到提交的中位时长。
- `request_id` 重试/恢复成功率。
- Pi 原生适配器 wait 成功率。
- 通用 MCP 手动 continuation 成功率。
- 因工作台未开启导致的失败率。

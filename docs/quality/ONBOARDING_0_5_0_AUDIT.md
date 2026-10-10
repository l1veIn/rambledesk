# 0.5.0 首次使用与外部适配器审计

2026-10-10，Windows x64；基线 `af99d74`（发布提交 `6144667` 之后），包含本次修复。以下为源码与隔离探针结果，不作为正式安装包、真实登录或模型访问的验收结论。

## 结论与问题

| 问题 | 复现与影响 | 本次处理 |
| --- | --- | --- |
| DeepSeek 首条消息失败后缺少配置入口 | `deepseek-acp@0.8.0` 在空 HOME、无 key 时 initialize、session/new 都成功；首条 prompt 返回 `-32603`，没有结构化 error data，未发模型请求。此前 Unknown 错误卡片不提供认证设置入口 | 对已知 DeepSeek 的 Unknown prompt 显示条件式首次设置说明、当前实际启动入口的 `--setup` 命令和高级设置入口；连接成功卡片也提供可展开说明。保留 Unknown，不把所有未知错误判成认证失败 |
| 外部 DSH 取消请求返回 HTTP 422 | 原生工具 schema 允许省略 `reason`，但 `/api/feedback/cancel` 要求必填；真实 HTTP 闭环失败，原有 mock 测试未覆盖 | 缺省或空白 reason 时发送 `Cancelled from dsh.`；保留用户提供的原因。单元回归和实际 DSH ToolRuntime 闭环通过 |
| 外部 DSH 已安装但未初始化时提示错误 | 官方 CLI 的 web/headless profile 在首次启动时初始化；CLI 已安装但 profiles 为空时，界面只有不可用的安装入口，后端提示先安装 dsh | 区分“未安装”与“尚未初始化”；提示先运行 `dsh web`，增加重新检测入口。交互回归覆盖初始化后刷新、发现 profile、允许安装；不自动启动或改写宿主配置 |
| 仅安装 dsh 与 ACP 就绪容易混淆 | 系统 `dsh` 与 `deepseek-acp` 是不同入口；首次 DeepSeek 连接优先准备应用自有组件，而非自动采用系统安装 | 保留现有选择策略；在使用指南中明确组件、认证、握手与首轮模型调用的区别 |

DSH 缺凭据行为与上游 [v0.8.0 登录说明](https://github.com/xintaofei/deepseek-acp/blob/v0.8.0/README.md#login)一致；[错误转译](https://github.com/xintaofei/deepseek-acp/blob/v0.8.0/src/index.ts#L167)没有保留可用于安全分类的结构化凭据错误码。因此修复落在可恢复操作和引导，不解析自由错误文本。

## 最近版本兼容性

- `v0.4.0..v0.5.0` 的 catalog、检测和安装逻辑没有改变。独立 `dsh` 不再作为 ACP 目录入口、首次 DeepSeek 优先托管安装的策略来自 `d631ea3`，在 0.4.0 已存在。
- Pi/DSH 外部适配器在这段版本中主要同步了 `what_happened` 200 字提示和版本号；外部传输、身份与等待模型仍沿用既有合同。201 字请求会明确拒绝，更正后能创建请求，不应静默截断。
- DSH 省略取消原因的问题自 `6475361` 引入原生适配器时已存在，并非 0.5.0 新增回归。
- 旧 Pi/DSH 原生工具继续暴露 `actions`；类型发现与新 `workbench` 入参目前通过 Generic MCP / ACP command 使用。原生工具不支持新增类型属于已声明能力缺口，不能声称完整功能对齐。
- 托管 ACP 环境抑制旧原生适配器，是防止重复反馈入口的设计；不能据此断言外部会话失效。

## 本次验证

| 场景 | 结果与证明边界 |
| --- | --- |
| 无 Node/npm 的新用户环境 | 检测为 missing，npm 检查失败，安装返回明确的 CommandUnavailable；没有发现父环境里的 Agent |
| 只有 Node/npm、空 HOME / APPDATA / npm 配置 | 真实安装 `deepseek-acp@0.8.0`、`codex-acp@1.8.0`，检测为 managed，实际版本与入口一致，ACP initialize 和 shutdown 通过；DSH session/new 通过 |
| DSH 无凭据首轮 | 真实组件 initialize/new 成功，prompt 在本地失败，0 次模型请求；重现上述引导问题 |
| DSH 合成 key + loopback SSE stub | 首轮收到预期回复标记和 end_turn，1 次 loopback 请求；仅证明协议接线，未验证真实模型 |
| 新手引导交互 | jsdom 覆盖无安装跳过、DSH 安装后握手失败重试、原生 ACP 首次扫描、打开首会话失败重试；不代表原生窗口人工操作 |
| 外部 DSH 首次安装引导 | 模拟已安装但无 profile，安装禁用且给初始化指引；重新检测发现 profile 后允许安装，不把前端 fixture 当作真实 CLI 首启证据 |
| 外部 Pi | 真实 HTTP/SQLite 上的 6 组闭环通过；注册仍使用 Pi API harness，未验证真实 Pi CLI 的加载和权限环境 |
| 外部 DSH | 实际 Cordis 4.0.1、DSH tools/commands/system-prompt 0.1.1-rc.2 加载插件，注册 4 tools/2 commands；实际 ToolRuntime.execute、输出 schema 与 6 组闭环通过 |
| Generic MCP | 较旧协议版本 2025-03-26 的客户端发现 5 tools，legacy create/get、typed questions 发现/创建/提交/结构化返回通过；不是各宿主 MCP 配置安装的证据 |

原生适配器闭环覆盖：附件、等待后提交与返回、get、相同 ID 重试、等待中断后重新加载与恢复、新 ID、取消，以及超长摘要拒绝后更正。DSH 与 Pi 使用同一持久服务，不以 mock HTTP 代替真实边界。

专项验证：ACP Rust 测试 81 项通过，3 项显式忽略（其中 2 项由隔离 runner 单独执行）；agents/onboarding 前端 43 文件、381 项通过；适配器设置交互回归 1 项、DSH installer 11 项、Pi 22 项、DSH 28 项通过；外部真实服务 17 检查组通过。Svelte 检查零错误、零警告。没有用这些数量代替上述场景边界。

隔离 runner 仅复制 Node 与 npm，不能把 Node 安装目录直接放进 PATH：开发机该目录可能同时含全局 Agent。HOME、USERPROFILE、APPDATA、LOCALAPPDATA、DSH_HOME、会话目录、npm 配置/cache/prefix 均独立；不继承账号或代理配置。首轮探针只允许本地 stub 网络入口，报告不保存凭据、请求正文或错误原文。

初轮探针因临时目录过深，使 Codex vendor exe 路径达到 267 字，触发 Windows 206 / ACP initialize 1001；同一份文件放到短路径后正常。这是测试环境问题，已缩短 runner 路径，未修改 Codex pin。真实超长用户目录的安装支持仍应单独验收。

## 可重复入口

Windows 上，从仓库根目录运行（每次使用新的输出目录）：

```powershell
python scripts/onboarding-isolated-smoke.py --output C:/temp/rd-smoke/catalog
node scripts/dsh-first-turn-smoke.mjs --catalog-report C:/temp/rd-smoke/catalog/catalog.json --output C:/temp/rd-smoke/dsh
```

前一个命令构建测试可执行文件，再用 allowlist 环境启动；保留二进制/源码 SHA-256、实际组件版本、阶段日志与结果。它使用进程隔离，不提供 OS、注册表或安装器隔离。缺少隔离前提时不得把普通开发机的成功解释成首次安装成功。

外部适配器使用[反馈验收夹具](FEEDBACK_ACCEPTANCE.md)：构建前端，启动全新 HTTP/SQLite fixture，再运行 `scripts/external-adapter-acceptance.mjs <manifestFile> <report.json> <isolated-dsh-node-modules>`，最后在 `finally` 中停止 fixture。输出目录仅保存报告，不归档凭证文件。

新增 `.github/workflows/onboarding-smoke.yml`，可手动触发 disposable GitHub-hosted Windows runner，执行上述三层验证、引导回归和两个适配器测试，失败时仍上传非秘密报告。本次未远端执行此工作流；它也不安装正式应用或操作原生 UI。

## 证据与剩余验收

本地通过报告为 `.local-artifacts/onboarding-isolated-20261010-111540-ebb9b228/report.json`、`.local-artifacts/dsh-first-turn-20261010/report.json` 和 `.local-artifacts/external-adapter-acceptance-2026-10-10.json`；源码与测试设施随本次变更保留，原始证据目录不提交。报告绑定基线提交、测试二进制或相关源文件哈希，不能沿用为下一构建的绿灯。较早的失败记录也保留，其中包括上文已排除的长路径问题。

发布前仍需独立干净 Windows 用户/VM 上的正式安装包首次启动、真实认证、真实模型首轮 → Ramble → 提交 → 继续，以及真实 Pi CLI 和各 Generic MCP 宿主的安装/加载。macOS 的同等流程也需另验。当前没有据此关闭这些项目，也没有发布新版本。

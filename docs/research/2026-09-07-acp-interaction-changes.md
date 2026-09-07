# Agent 检测缓存与会话交互修复

本次继续在 `codex/acp-managed-sessions` 工作区实施，保留已有未提交改动，未提交或推送。完整的 16 个 Agent 能力及来源见 [调研报告](2026-09-07-acp-agent-capabilities.md)。

## 已实现

| 范围 | 行为 |
| --- | --- |
| 检测缓存 | 设置、新手引导、新建会话共享同一应用连接的内存缓存。页面关闭后完成的检测仍可复用；重新打开页面不启动新扫描。较早检测、旧运行实例、已变更或删除的启动配置不能覆盖当前结果。 |
| 状态含义 | 成功标记为「上次检测通过」，区分历史握手结果与实时会话连接。手动检测更新结果；启动配置变化和运行实例更换使相关结果失效。 |
| Grok 配置 | 读取实际公布的 `x.ai/sessionConfig` 与模型 metadata，提供模型及思考强度选择；标准 config options 优先，按模型更新思考选项，切换失败保留原值。其 effort 不作为权限模式。 |
| 工具授权 | 沿用标准 permission 队列和 Agent 提供的原始 option ID，补齐拒绝观察事件时的 responder 清理。 |
| 结构化问答 | 新增 ACP `elicitation/create`、Grok `_x.ai/ask_user_question`、Cursor `cursor/ask_question`。支持常见单选、多选、文本、布尔和数字字段，回答回到原 JSON-RPC 调用。 |
| 计划审批 | 新增 Grok `_x.ai/exit_plan_mode` 和 Cursor `cursor/create_plan`，要求用户显式选择批准；保留各自的取消/拒绝结果。 |
| 请求生命周期 | 错误答案不消费请求；跨会话、重复回答被拒绝；取消、断线、轮次结束后的迟到请求不留下无界等待。暂不支持的表单约束保留可见卡片及拒绝/取消入口。 |

没有添加按 Agent 品牌或「YOLO」文字自动批准请求的逻辑。模式语义和选项以当前 Agent 实际公布的能力为准。

## 验证

- 前端：129 个测试文件、870 项测试通过；Svelte/TypeScript 检查 0 错误、0 警告。
- Rust core/storage/local-server：248 项测试通过。
- ACP：57 项通过，2 项按既有配置忽略（辅助 fixture 与网络安装集成）；最终输入校验定向回归再次通过。
- `cargo clippy -p rambledesk-acp --all-targets -- -D warnings`、Rust 格式与模块大小检查、生成契约检查、`git diff --check` 通过。
- Web production build 通过，保留已有大 chunk 提示。
- 桌面原生端 `cargo check -p rambledesk-desktop --target-dir target/desktop --quiet` 通过。
- 隔离浏览器使用真实会话组件验证：必答校验、单/多选和补充文字、提交后恢复输入、取消提问、切换页面后缓存仍在且检测计数不增加。

协议测试使用本地 Node stdio fixture，校验实际请求/响应包；没有使用真实账号向厂商模型发送任务。以上不能证明 16 个 Agent 的所有版本、模型和认证环境均已完整可用。

## 已知边界

- 缓存仅在当前应用进程/连接中共享，不写入磁盘。启动签名含环境变量值，不能直接序列化；重启应用后仍需重新检测。
- 新增表单能力在 Agent 重新连接时协商，已有运行实例不会自动获得新能力。
- 只接入活动会话的表单问答；URL 与创建会话前的认证问答未接入。
- 正则、格式等暂不支持的复杂 schema 不能提交接受，但用户可以在卡片中拒绝或取消。
- Grok 的 `keep_planning` 忽略 feedback；界面提示继续规划后另发修改要求，并拒绝会被忽略的组合，避免静默丢失说明。
- Grok 私有扩展参考 Codeg 固定 commit 的客户端实现；其历史实测版本与当前 catalog 版本不同，具体版本依据见调研报告。

开发预览：`pnpm -C apps/desktop exec vite --host 127.0.0.1 --port 1431`，打开 `/agent-preview.html?question`，选择「项目会话」。预览仅使用内存数据。

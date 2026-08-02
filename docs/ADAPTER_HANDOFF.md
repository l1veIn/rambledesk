# 专有 Adapter 开发交接（简易报告）

> 日期：2026-08-01
> 受众：下一任 agent / 实现者
> 状态：方向已定，通用路径已落地；专有 adapter 尚未实装
> 相关：`docs/AGENT_SUSPENSION.md`、`crates/rambledesk-adapters/`、`apps/desktop/src-tauri/src/mcp_setup.rs`

---

## 1. 背景与目标

RambleDesk 的 **数据面**（创建请求、落盘 Package、`get_feedback`）已通过 MCP 短调用完成。
真正缺口在 **控制面**：人类提交反馈后，如何让原来的 coding agent **继续同一任务**，而不是用户手打「好了」。

当前默认闭环：

```text
request_feedback → Agent 结束 turn（不轮询、不长 wait）
  → 人类在桌面 ramble 并提交
  → WakeupRouter.wake(host_id)
  →（现仅）Generic → 弹窗 + 可复制 resume 提示
  → 用户回宿主粘贴 / 触发继续
  → get_feedback(request_id) → 继续任务
```

下一阶段目标：在 **支持自动续聊的宿主** 上增加 **专有 adapter**，提交后尽量 **零点击 / 少点击** 唤醒；其余宿主继续走通用路径。

---

## 2. 通用 Adapter vs 专有 Adapter

| 维度 | 通用（Generic） | 专有（Host-specific） |
|------|-----------------|------------------------|
| 代码位置 | `rambledesk-adapters` 内 `GenericWakeupAdapter` | 同 crate，每宿主一个实现 |
| 匹配条件 | host 为空 / `unknown` / **无任何** `matches_host` 命中 | `matches_host("claude" \| "codex" \| …)` |
| 提交后行为 | `WakeResult::UserPrompt`：聚焦主窗 + 对话框 + 复制 resume 文案 | `WakeResult::HostDelivered`：调用该宿主 resume / 注入 turn |
| 用户操作 | 通常需回宿主粘贴提示或口头「继续」 | 理想：自动开新 turn 或注入上下文 |
| 依赖 | 仅 RambleDesk UI | 宿主公开/半公开 API、插件、凭据、continuation id |
| 失败策略 | 本身即兜底 | **必须可降级** 到通用弹窗，不得丢 completed 结果 |
| 变更节奏 | 稳 | 随宿主版本常变 → **禁止塞进 `rambledesk-core`** |

**共同点（硬约束）：**

- 业务主键永远是 `request_id`；wake 通道 **不携带** Package 正文。
- Agent 恢复后必须 `get_feedback`（或等价短读）取 canonical package。
- MCP `wait_for_feedback` **不是** 默认等人路径（已从 MCP 工具面移除）。

**一句话：**

- **通用** =「我叫不醒 Agent，但帮你备好恢复话术。」
- **专有** =「我认识这家宿主，尽量直接把会话续上；不行就交给通用。」

---

## 3. 现有代码地图（给下一任定位）

| 路径 | 职责 |
|------|------|
| `crates/rambledesk-adapters/` | `WakeupRouter`、`WakeupAdapter` trait、`GenericWakeupAdapter`、`WakePayload` / `WakeResult` / `ResumePrompt` |
| `apps/desktop/src-tauri/src/lib.rs` | `submit_feedback` 后 `deliver_wakeup_after_terminal`；`UserPrompt` → emit `rambledesk://resume-prompt` |
| `apps/desktop/src/App.svelte` | 恢复提示对话框 UI |
| `apps/desktop/src-tauri/src/mcp_setup.rs` | **仅** MCP 配置写入 + `RAMBLEDESK_HOST` / `X-RambleDesk-Host` |
| `crates/rambledesk-mcp/` | 工具仅 `request_feedback` / `get_feedback` / `cancel_feedback`；header 可覆盖 `agent` |
| `docs/AGENT_SUSPENSION.md` | 挂起语义、数据面/控制面、能力分级 A–D |

Desktop 组装处目前类似：

```rust
wakeup: WakeupRouter::default()  // specific 列表为空 → 全部走 generic
```

专有 adapter 实装后应改为例如：

```rust
WakeupRouter::new(vec![
  Arc::new(ClaudeWakeupAdapter::from_config(...)),
  Arc::new(CodexWakeupAdapter::from_config(...)),
])
```

---

## 4. 产品侧重要升级（尚未做，但方向已定）

当前 Settings 是 **「MCP 一键注册」**。
应升级为 **「宿主适配（Host Integration）」**：

- **适配 ≠ 只写 MCP server 条目。**
- 不同专有 adapter 对应 **不同适配流程**（步骤可含：MCP、插件、extension、凭据、continuation 登记、健康检查）。
- 适配成功应区分能力：
  - **数据面就绪**：能 `request` / `get`
  - **控制面就绪**：提交后能 `HostDelivered`，否则明确 **仅通用弹窗**

建议抽象（实现时可放 `rambledesk-adapters` 或紧邻模块）：

```text
HostIntegration
  detect() / plan() / run_step()
  capabilities()  // mcp, auto_wake, needs_plugin, ...
  wakeup_adapter() -> Option<WakeupAdapter>
```

现有 `mcp_setup` 应收成某宿主 plan 里的 **一步**（`EnsureMcp`），而不是整个产品入口。

---

## 5. 专有 Adapter 开发清单（建议顺序）

对 **每一个** 目标宿主单独做，不要假设矩阵全绿。

### 5.1 调研（阻塞实现）

1. 是否存在 **进程外** 可调用的续 turn / 注入消息 API？
2. 是否需要插件 / extension 才能挂 continuation？
3. session / thread / resume token 从哪来？能否在 `request_feedback` 时登记？
4. 机器休眠、宿主退出、8 小时后是否仍可 wake？
5. 鉴权与安全：loopback、token、最小权限、可撤销

能力分级参考 `AGENT_SUSPENSION.md` 的 A–D：

| 等级 | 含义 | 适配结果 |
|------|------|----------|
| A | 可外部触发新 turn | 专有 wake 主路径 |
| B | 可发消息但 turn 不保证 | 尽力 HostDelivered + 保留 UserPrompt 兜底 |
| C | 仅 CLI/MCP | 只做数据面 + 通用弹窗 |
| D | 几乎无本地接入 | 人工复制 request/package |

**无 A/B 证据的宿主：不要做假专有 adapter。**

### 5.2 实现（有证据后再写代码）

1. 在 `rambledesk-adapters` 新增 `src/hosts/<host>.rs`（或等价模块）。
2. 实现 `WakeupAdapter::{id, matches_host, wake}`。
3. `wake` 成功 → `HostDelivered`；失败 → **不要吞掉**，上层降级 `GenericWakeupAdapter`（桌面已会展示 UserPrompt 的路径需接好）。
4. 扩展适配流程（Settings）：该宿主的 `IntegrationPlan` 步骤。
5. 测试：单元（matches / 降级）+ 该宿主手工 dogfood（提交 → 是否自动续 → `get_feedback`）。
6. 文档：COMPATIBILITY / 本 handoff 更新该宿主等级。

### 5.3 建议优先顺序（可调整）

以 **dogfood 频率 + 公开续聊能力** 排序，首家务必选 **有明确外部 trigger 证据** 的宿主。
候选：Pi（`triggerTurn` 等）、OpenCode（async prompt 类）、Codex App Server、Claude（需再实测，勿假设）。
**Grok / Cursor 等先按 C 级：MCP + 通用即可。**

---

## 6. 不变量与禁止项（下一任勿破坏）

1. **`rambledesk-core` 不放宿主 resume 实现**（节奏不同；已从 core 拆出 adapters）。
2. **默认 MCP 工具保持 3 个**：`request_feedback` / `get_feedback` / `cancel_feedback`。不要把长 `wait` 加回默认路径。
3. **timeout 只结束一次调用 attempt，不取消 Feedback Request。**
4. **自动唤醒失败不得丢失 completed**；Inbox + `get_feedback` 永远可恢复。
5. **host id** 与 MCP 自动注册的 `RAMBLEDESK_HOST` / `X-RambleDesk-Host` 对齐（`claude` / `codex` / `cursor` / `gemini` 等）。
6. **Skill 只描述工作流，不靠软约束完成闭环**；专用路径应硬触发 turn。
7. 适配 UI 勿再暗示「装了 MCP = 全自动续聊」。

---

## 7. 验收标准（专有 adapter 做完一家算过）

- [ ] 该宿主 `matches_host` 仅命中自己的 id。
- [ ] 提交 completed 后，在 **已登记 continuation** 的前提下，Agent 无需用户手打「好了」即可进入新 turn（或产品声明的 B 级行为）。
- [ ] 故意破坏 wake（错误 token / 宿主未开）→ **自动降级** 通用弹窗，Package 仍可通过 `get_feedback` 读取。
- [ ] 未装该 adapter 的宿主行为与现在一致（纯 generic）。
- [ ] 适配流程文档/UI 写清：做了哪些步骤、能力是自动 wake 还是仅弹窗。

---

## 8. 给下一任的最短路径

1. 读本文件 + `docs/AGENT_SUSPENSION.md` + `crates/rambledesk-adapters/src/wakeup.rs`。
2. 选定 **一家** 宿主，先做 **续聊能力实测**，再写代码。
3. 实现 `WakeupAdapter` 并挂入 desktop 的 `WakeupRouter::new(...)`。
4. 把 Settings 的「MCP 安装」演进为该宿主的 **适配 plan**（MCP 只是一步）。
5. 测通：提交 → wake → `get_feedback` → 任务继续；再测 wake 失败降级。

---

## 9. 当前仓库事实快照（防误解）

- MCP 默认工具：**3 个**（无 health / wait / list 给 Agent）。
- 提交后 **已有** 通用恢复提示 UI。
- **专有 adapter 数量：0**。
- `mcp_setup` 仍是统一 MCP 写入，**尚未**升级为 per-host Integration Plan。
- 工作区若仍有未提交的截图 WIP，与 adapter 工作无关，勿混进同一 PR。

---

*本报告仅作交接；实现细节以代码与宿主实测为准。*

# ADR 005：Unified Workbench 中保留两个隔离的 Session Source

- 状态：Accepted
- 日期：2026-08-31

## 背景与决策

ACP 为 RambleDesk 带来主动 Launch、runtime 状态与配置、Permission、Ask 和可恢复 Feedback Delivery，但既有 MCP、Pi 与原生 Adapter 仍持有可用数据和原操作。强制 ACP-only 替换会要求用户先做有损迁移，并在 ACP 能力尚未覆盖旧路径时失去已有工作流。

因此 Desktop 以 Unified Workbench Projection 合并 Managed ACP Session 与 Adapter Session，但不合并它们的 Core 或存储：v3 Core 只拥有 Managed ACP/Imported 事实，Adapter Runtime 继续拥有既有事实；命令按 Session Source 返回原 owner，禁止双写与跨 Core fallback。Adapter 路径维护冻结但保持可达，有损迁移是显式可选项；未来是否进一步收敛必须基于逐能力验收、迁移质量和真实使用证据另作决策。所有规范术语只在 [TERMINOLOGY.md](../TERMINOLOGY.md) 定义。

## 后果

Desktop 必须维护 source-aware identity、projection 与 command routing，并测试 id 碰撞和单 source 故障；作为交换，用户可以在同一工作台直接使用旧数据，同时新 ACP 能力不被旧模型约束或伪装为功能齐平。

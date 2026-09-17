# 工作台与公共容器的接口（讨论稿）

状态：**讨论稿，未实现**。本文只定义"工作台"与"公共容器 / 反馈列"之间可能的最小接口。
运行合同仍以[架构](../ARCHITECTURE.md)、[协议](../PROTOCOL.md)、[术语表](../TERMINOLOGY.md)为准。

## 1. 为什么需要接口

现在工作台与反馈列之间有三处隐式耦合，工作台一多就会各自膨胀：

1. `SessionWorkbench` 用 60 多个 prop 把容器、编辑器、采集、提交全透传给工作台；
2. "当前写到哪里"藏在 `draftOperationsController` 的 `activeActionByRequest` 里，
   采集管线（语音 / 截图 / 剪贴板 / 文件）在写入时各自回头查询；
3. 提交与结果拼装分散在容器、控制器与编辑器三处。

## 2. 接口草案

```ts
/** 工作台拿到的全部输入。没有编辑器、没有 transport、没有 store。 */
export type WorkbenchContext = Readonly<{
  request: WorkbenchRequestView      // 请求材料的只读投影（含 what_happened、actions、材料附件）
  data: unknown                      // 该类型的专用数据（由注册项解析，见 registry-and-data.md）
  readOnly: boolean
  locked: boolean                    // 提交 / 整理 / 取消 / 批准进行中
  activeTargetId: string | null      // 容器持有的"当前输入目标"（见第 3 节）

  onTargetsChange(targets: readonly WorkbenchTarget[]): void   // 声明可写入对象
  onSelectTarget(targetId: string | null): void                // 用户选中了谁
  onResultChange(result: WorkbenchResult | null): void         // 结构化结果（可选能力）

  formatTime(value: string | null | undefined): string
}>
```

对应地，容器负责：一个可编辑编辑器、采集唯一入口（且采集显式携带目标）、草稿与保存、
提交 / 取消 / 批准 / 发布、身份行渲染、以及类型未知或版本不支持时的降级。

约束：

- 工作台不 import Application Transport、Tauri、store，不写协议，不拼反馈包，不直接碰编辑器；
- 容器不知道自己是哪种工作台，也不拥有某个工作台特有的数据；
- 共享呈现（目标 chip、条目容器、答案字段、空态）放在公共模块，新工作台是"组合"而不是"复制基础设施"。

## 3. "输入目标"是这套接口的核心

成熟做法（代码评审线程、设计评论、文档批注、走查表、可用性测试）都是同一句话：
**输入挂在对象旁边，汇总发生在提交时**。落到这里就是：

- 工作台**声明目标**（Ramble：体验动作；将来：题目、卡片）；
- 容器**持有当前目标**（按请求、随草稿持久化），并在工具栏显示"写入：② …"的 chip；
- 采集开始时固定 `{requestId, targetId}`，异步结果回来再校验目标与内容版本；
- 文档里的 `Action Group` 只是**投影**，不是目标状态的真相来源。

这样"就地输入"的手感来自目标 chip + 条目旁的麦克风按钮，而编辑器、采集、提交仍然只有一份——
不需要给每个条目复制一个 TipTap（那会同时破坏"一个编辑器"、采集单实例与协议承载能力）。

## 4. 目录落点与边界强制

```text
lib/workbench-contract/     契约类型 + 纯函数（目标与结果的校验、投影）
lib/workbenches/<type>/     各工作台：一个入口组件 + 声明
lib/workbench/              容器、反馈列、控制器（工作台不可 import）
```

- `lib/workbenches/*` 允许 import：`lib/(root)`、`lib/domain`、`lib/components`、`lib/workbench-contract`；
- 禁止：`lib/workbench`（容器实现）、`lib/capabilities`、`lib/application`、`@tauri-apps/*`；
- 用 `frontendBoundaries.test.ts` 的 lockfile 机制加一条规则，并配合模块体积检查，
  让"工作台很轻"成为可执行约束。

## 5. 迁移顺序（每步都能单独上线）

| 步骤 | 内容 | 验收 |
| --- | --- | --- |
| **A** | 把容器的 60 多个 prop 收敛成 `WorkbenchContext`；目标所有权从 `draftOperationsController` 移到容器；Ramble 工作台改为经上下文交互 | 行为完全不变；现有测试 + 新增边界测试通过 |
| **B** | 静态注册表 + 注册默认工作台；未知类型与版本降级 | 旧请求保持可读可编辑可提交；新增第一方类型不必复制容器 |
| **C** | 协议加 `workbench` 与结构化结果；用第二个工作台验证接口 | **两种工作台都跑通后再冻结接口** |

暂不做（等真实需求）：远程安装、动态代码加载、通用组件 DSL、第三方工作台市场。

## 6. 待定

1. 接口面是否够小（一个只读上下文 + 三个回调）？还是结果也由容器从工作台状态里取（更小但更隐式）？
2. 身份行（Host / 会话 / 状态 / 标题）由容器统一渲染、工作台只给标题与摘要，可以吗？
3. 输入目标是"容器持有的单一目标"，还是允许工作台声明多个并行目标（例如一次性给三道题各写一段）？
4. 更多问题见[开放问题](open-questions.md)。

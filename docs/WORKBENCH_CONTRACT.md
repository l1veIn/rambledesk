# 工作台契约（提案草稿）

状态：**提案，尚未实现**。本文只定义"工作台"与"公共容器 / 通用反馈列"之间的接口；
运行合同仍以[架构](ARCHITECTURE.md)、[协议](PROTOCOL.md)、[术语表](TERMINOLOGY.md)为准，
迭代路线见 [WORKBENCH_EVOLUTION.md](WORKBENCH_EVOLUTION.md)（阶段 2–4）。

目的一句话说：**工作台可以有很多（几十上百），所以工作台只写"内容与结果"，其余一律通过契约交给容器。**

## 1. 为什么要契约，而不是让工作台直接用反馈列

现在工作台与反馈列之间的耦合有三处是"隐式"的：

1. `SessionWorkbench` 用 **60 多个 prop** 把容器、编辑器、采集、提交全透传给工作台；
2. "当前写到哪里"（`activeActionByRequest`）藏在 `draftOperationsController` 里，
   采集管线（语音/截图/剪贴板/文件）在写入时各自回头查询；
3. 提交与结果拼装分散在容器、控制器与编辑器三处。

工作台一多，这三处会各自膨胀成"每个工作台一份"。契约的目标是把它们收敛成**一个只读上下文 + 三个回调**。

## 2. 契约形状（草案）

```ts
/** 工作台拿到的全部输入。没有编辑器、没有 transport、没有 store。 */
export type WorkbenchContext = Readonly<{
  /** 请求材料：标题、摘要、材料附件、体验动作等的只读投影。 */
  request: WorkbenchRequestView
  /** 该工作台的专用数据（版本化，由注册项解析）。 */
  data: unknown
  readOnly: boolean
  /** 提交 / 整理 / 取消 / 批准进行中：工作台只能展示，不能改数据。 */
  locked: boolean
  /** 容器持有的"当前输入目标"（见第 4 节）。 */
  activeTargetId: string | null

  /** ① 声明目标：容器据此渲染目标 chip 与就地输入入口。 */
  onTargetsChange(targets: readonly WorkbenchTarget[]): void
  /** ② 用户在工作台里选中/取消某个目标。 */
  onSelectTarget(targetId: string | null): void
  /** ③ 结构化结果（纯数据，随草稿保存；没有结构化结果的工作台不用）。 */
  onResultChange(result: WorkbenchResult | null): void

  formatTime(value: string | null | undefined): string
}>

export type WorkbenchTarget = Readonly<{
  /** 稳定 id，来自请求数据，不由界面生成。 */
  id: string
  label: string
  /** 展示顺序（Ramble：体验动作的序号）。 */
  index?: number
}>

export type WorkbenchResult = Readonly<{
  version: number
  answers: readonly WorkbenchAnswer[]
}>

export type WorkbenchAnswer = Readonly<{
  itemId: string
  kind: 'text' | 'choice' | 'skipped'
  text?: string
  choices?: readonly string[]
}>
```

注册项（阶段 3 才落地，形状先在这里定下来，避免工作台各自发明）：

```ts
export type WorkbenchDefinition = Readonly<{
  type: string          // 'ramble' | 'ask-question' | …
  version: number       // 交互/组件版本
  dataVersion: number   // 专用数据版本
  name: string
  description: string
  /** 未知类型或版本不支持时的降级入口：能读材料、能自由反馈、明确提示。 */
  parseData(raw: unknown): { ok: true; data: unknown } | { ok: false; reason: string }
  /** 只接收 WorkbenchContext 的组件。 */
  view: Component<{ context: WorkbenchContext }>
  /** 结果 → 可读 Markdown 投影（写给 Agent 与人看的摘要）。 */
  projectMarkdown?(result: WorkbenchResult): string
}>
```

## 3. 各自的职责（写清楚就不会耦合）

**容器（唯一实现，工作台不复制）**
- 一个可编辑编辑器；采集（语音 / 截图 / 剪贴板 / 文件 / Tidy）唯一入口，且**采集显式带目标**；
- 草稿读写、自动保存、草稿 CAS/revision；
- 提交、取消、批准、发布、附件打包，以及"未回答 / 跳过 / 仅自由反馈"的语义；
- 工作台身份行（Host / 会话 / 请求状态 / 标题）统一渲染；
- 目标消失、请求结束、类型未知、版本不支持时的降级行为。

**工作台（每个工作台只写这些）**
- 渲染 `context.data` 的内容（材料、条目、表单…）；
- 通过 `onTargetsChange` 声明"哪些对象可以被写入"；
- 通过 `onSelectTarget` 上报用户选择；
- 通过 `onResultChange` 上报结构化结果（纯数据）；
- 不 import transport / Tauri / store，不写协议，不拼反馈包，不直接碰编辑器。

**共享呈现**放在一个公共模块（`lib/components` 或新的 `lib/workbench-ui`）：
目标 chip、条目容器、答案字段、空态等。新工作台 = 组合这些，而不是复制基础设施。

## 4. "输入目标"是这套契约的核心

成熟做法（GitHub 评审线程、Figma 评论、Docs 批注、QA 走查表、可用性测试）都是同一句话：
**输入挂在对象旁边，汇总发生在提交时**。落到我们这里就是：

- 工作台**声明目标**（Ramble：体验动作；AskQuestion：题目；将来的卡片/选项同理）；
- 容器**持有当前目标**（按请求、随草稿持久化），并在工具栏显示"写入：② …"的 chip；
- 采集开始时就固定 `{requestId, targetId}`，异步结果回来再校验目标与内容版本；
- 文档里的锚点只是**投影**（这段文字属于哪个目标），不是真相来源；只在真的写入内容时出现。

这样"就地输入"的手感来自目标 chip + 每个条目旁的麦克风按钮，而**编辑器、采集、提交仍然只有一份**——
不需要给每个条目复制一个 TipTap（那会同时破坏"一个编辑器"、采集单实例与协议承载能力）。

## 5. 数据回传（结果）的三层

| 层 | 归属 | 形态 |
| --- | --- | --- |
| 自由反馈 | 容器 | TipTap 正文 + 附件（今天的协议不变） |
| 工作台结构化结果 | 工作台声明，容器保存与提交 | `WorkbenchResult`（纯数据，版本化） |
| 可读投影 | 容器 | Markdown：正文 + 每个目标的结果摘要 |

阶段 4 才动协议：请求侧加 `workbench: { type, version, data }`，结果侧加 `answers[]`；
先改 Rust 源类型再生成 TypeScript，幂等比较、草稿 CAS、未知类型降级按路线文档第 6 节一起定。

## 6. 目录与边界（用现有机制强制）

```text
lib/workbench-contract/     契约类型 + 纯函数（目标/结果的校验与投影）
lib/workbenches/ramble/     第一个工作台：只要一个入口组件 + 声明
lib/workbenches/ask-question/
lib/workbench/              容器、反馈列、控制器（工作台不可 import）
```

- `lib/workbenches/*` 允许 import：`lib/(root)`、`lib/domain`、`lib/components`、`lib/workbench-contract`；
- 禁止：`lib/workbench`（容器实现）、`lib/capabilities`、`lib/application`、`@tauri-apps/*`；
- 用现有 `frontendBoundaries.test.ts` 的 lockfile 机制加一条 `lib/workbenches -> …` 规则，
  再加一个"每个工作台目录只有一个入口组件、总行数有上限"的体积检查，
  让"工作台很轻"成为可执行约束，而不是口号。

## 7. 迁移顺序（每步都能单独上线）

| 步骤 | 内容 | 验收 |
| --- | --- | --- |
| **A（现在）** | 把容器的 60 多个 prop 收敛成 `WorkbenchContext`；目标所有权从 `draftOperationsController` 移到容器；Ramble 工作台改为三个回调交互；加边界测试 | 行为完全不变；现有测试 + 新边界测试通过 |
| **B（阶段 3）** | 静态注册表 + 注册 Ramble；未知类型/版本降级；快照与页签按 type 显示 | 旧请求保持可读可编辑可提交；新增第一方类型不用复制容器 |
| **C（阶段 4）** | 协议加 `workbench` 与 `answers[]`；AskQuestion 作为第二个注册项 | **两种工作台都跑通后再冻结契约**（这才是契约的验证） |

暂不做（等真实需求）：远程安装、动态代码加载、通用组件 DSL、第三方工作台市场。

## 8. 待确认

1. 契约面是否够小（一个只读上下文 + 三个回调）？还是结果也由容器从工作台状态里取（更小但更隐式）？
2. 身份行（Host / 会话 / 状态 / 标题）由容器统一渲染，工作台只提供"标题与摘要"，可以吗？
3. 目录落点 `lib/workbenches/<type>/` + `lib/workbench-contract/` 可以吗？
4. 先按本文写一份 ADR/文档定稿，还是直接做步骤 A（行为不变的重构）？

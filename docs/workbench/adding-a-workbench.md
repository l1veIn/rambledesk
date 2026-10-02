# 新增普通工作台

普通工作台使用请求材料、结构化状态和业务视图，复用宿主的正文、草稿、输入、发布及按需开启的全屏页签。评分、排序、选择和带意见框的表单属于这个范围。需要新进程、浏览器桥接、设备或传输命令的类型，还要实现运行资源适配器；终端是这种类型的参考。

## 1. 生成可运行模板

在仓库根目录执行：

```sh
pnpm workbench:new proposal_review --dry-run
pnpm workbench:new proposal_review
pnpm contracts:generate
cargo fmt --all
pnpm check
```

名称使用唯一的 `snake_case`，每段以字母开头。脚手架拒绝已有文件、已注册名称、目录路径和保留字。`--dry-run` 显示将创建和修改的文件，不写入。

模板是一套可工作的“1 至 5 分＋可选意见”业务。开发者随后在自己的模块内改成实际需求。

正式的 `sort` 工作台也是从这份脚手架生成的：Rust 定义条目与完整排列，前端 definition 负责校验，视图复用已有拖动组件；它没有新增宿主、字段、传输或保存分支。可以参考 `crates/rambledesk-core/src/workbenches/sort.rs` 和 `apps/desktop/src/lib/workbench/definitions/sort/`，体验材料位于 `playground/workbenches/materials/sort.md`。

脚手架创建以下六个文件：

| 位置 | 用途 |
| --- | --- |
| `crates/rambledesk-core/src/workbenches/proposal_review.rs` | 强类型 Data / State / Result、发现/schema/example、输入与结果规则、业务测试 |
| `apps/desktop/src/lib/workbench/definitions/proposal_review/definition.ts` | 输入识别、状态解码、完成条件、布局、字段适配器、预览示例和视图绑定 |
| 同目录 `View.svelte` | 业务展示与编辑，通过 `context.host` 修改状态；启用全屏时提供宿主入口 |
| 同目录 `definition.test.ts` | 完成条件、字段替换和材料失效测试 |
| `playground/workbenches/fixtures/proposal_review.json` | 正常请求入口的体验输入 |
| `playground/workbenches/materials/proposal_review.md` | 人工操作说明 |

**脚手架只改两份已有共享生产文件**：Rust 的 `workbenches/registry.rs` 和前端的 `definitions/registry.ts`。DTO、状态联合、目录及分发代码从注册声明生成。生成的 `feedback.ts` 需要重新生成，不手工编辑。预览按 definition 的 examples 发现类型，playground 按 fixture 文件发现类型。

## 2. 填写业务合同与规则

Rust 模块中明确区分：

- **Data** 是不可变请求输入，例如待评审材料。
- **State** 允许未完成编辑，例如尚未选择的评分。普通草稿保存不会要求评分已完成。
- **Result** 是可以发布的结果，例如合法评分与意见。

模块的 `definition()` 提供发现元数据和公共策略；`validate()` 验证输入；`draft_valid()` 验证可保存状态；`result()` 将 State 投影为 Result；`has_input()` / `result_has_input()` 判断有效输入；`complete()` 决定发布条件；`legacy_actions()` 通常返回空数组。不要把自己的业务实体投影成 Action，仅为复用旧 Action 流程。

`strict_state` 拒绝损坏的已存在状态，仍允许没有状态或合法未完成状态。`require_complete` 默认开启，右侧补充正文无法绕过业务必填项。schema 注解与实际校验一起维护，后端是最终裁决者。

Rust 注册项声明模块、wire 身份、Data、State、Result 和 DTO 导出清单。普通类型可直接使用模板的 tuple 变体；序列化 State 仍是带 `type` 的扁平对象。已有类型的旧结构变体保留，避免改变历史合同与 Rust 调用代码。

## 3. 编写 definition 与业务视图

前端 definition 负责 `accepts`、`decodeState`、`hasInput`、`complete`、可选提交提示、`layout`、`fields`、`loadView` 和 `examples`。解码允许合法未完成状态，完成条件与后端一致。视图通过懒加载回调导入，纯规则与后台输入无需载入 Svelte。

`View.svelte` 只接收 `WorkbenchViewContext`：工作区、状态、锁定/只读条件及宿主能力。通过 `context.host.updateState(next)` 更新状态，`quote(text)` 引用到正文。公共材料、正文、保存队列和发布由宿主提供。历史使用同一视图的只读模式。

全屏是本类型的可选配置：普通模板的 `layout.expanded` 默认是 `false`，简单选择和排序保持关闭。网页、终端或其他需要宽屏工作区的类型可设为 `true`；宿主才会提供 `context.host.openExpanded`，调用它会在新页签继续同一请求。模板保留由 `{#if context.host.openExpanded}` 门控的按钮，主动开启后即可使用，不需要修改公共宿主代码。

业务文本字段提供 `WorkbenchFieldAdapter`：识别目标，读取值，声明长度/合同/身份，返回强类型替换函数，读取语音来源文本，并清除被删除的附件引用。业务字段身份使用 `workbench_field`，包含 type/version/field/entityId 和可选来源版本，不能用 DOM 焦点或任意 JSON 路径代替。

模板使用 `WorkbenchTextField`、`workbenchFieldVoiceTarget`，并在可编辑视图首次挂载时初始化空 State，使第一笔语音或附件也能找到字段。来源版本标记需符合目标长度上限；完整材料合同仍用于异步写回校验。等待期间材料、请求或目标失效时拒绝写入，不转移到正文。不要另建录音器、回执或整理队列。

`examples` 在本类型内声明预览内容和附件。默认普通工作台不需要 controller；运行资源类型可提供 `createController`，通过请求级 `prepareSubmission/dispose` 和视图级 `attach/detach` 管理独立生命周期。所有提交入口调用同一控制器，后端的 `pre_publish` 仍需按真实资源状态验证。

## 4. 验证并体验

```sh
cargo test -p rambledesk-core proposal_review
pnpm -C apps/desktop test src/lib/workbench/definitions/proposal_review
pnpm test:workbench-tools
pnpm test:workbench-extension
pnpm contracts:check
pnpm check
node playground/workbenches/prepare.mjs check proposal_review
pnpm dev:web
```

打开 `http://127.0.0.1:1420/workbenches.html?type=proposal_review`。完整应用预览使用 `?preview=fixtures&workspace=proposal_review`。这些页面模拟持久化或输入能力，用于开发；真实闭环还需在含新类型的后端，通过正常发现、请求、保存和发布入口体验。

在 playground 目录的 RambleDesk Agent 会话中按 `PLAYGROUND.md` 的身份与恢复规则操作：先发现类型，再用 `node prepare.mjs new proposal_review` 准备该类型，发送本轮生成的请求，交给用户体验后继续。脚本只准备文件，不发送请求或代填反馈。已有未完成运行必须恢复或取消，不能覆盖。

除本类型业务测试，还检查保存/恢复、正文与字段并发、无正文提交、未完成结果拒绝、只读历史和未知版本保留。仅对 `layout.expanded: true` 的类型验收全屏往返、同一份草稿和左侧会话列表；关闭时检查没有全屏入口。麦克风、截图、原生权限要在实际设备上验证，模拟输入不能代替。

`test:workbench-extension` 临时写入两个注册项和生成合同，结束后恢复，不保留测试产品类型。运行时停止其他源码修改、合同生成或构建任务，避免并发写入覆盖恢复的文件；生成名称 `extension_probe` 已被占用时会在写入前拒绝。

## 仅测试的扩展演练

仓库保留 `rating_review`，用于验证新增流程，不是生产类型。新建其他开发用例时可加 `--fixture`；两个注册项会同时受开发条件限制，JSON 放入 `fixtures/development/`。

```sh
cargo test -p rambledesk-storage --features workbench-fixtures rating
node playground/workbenches/prepare.mjs check rating_review --development
```

PowerShell 中启动开发应用：

```powershell
$env:VITE_WORKBENCH_FIXTURES = '1'
pnpm -C apps/desktop tauri dev --features rambledesk-core/workbench-fixtures
```

只启动前端时设置同一环境变量再运行 `pnpm dev:web`。生产 Vite 构建无论是否设置该变量，都不会注册或打包开发类型的视图；默认 Cargo 构建也不包含该类型。合同生成命令开启开发 feature，保留开发测试所需的 DTO 声明；这些擦除后的 TypeScript 类型不增加生产业务代码。

实际演练记录见 [框架验收](framework-validation.md)。

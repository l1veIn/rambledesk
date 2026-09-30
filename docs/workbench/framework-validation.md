# 工作台框架验收

日期：2026-09-30。分支：`codex/workbench-framework`。对应[设计目标](framework-plan.md)与[新增教程](adding-a-workbench.md)。

## 实际扩展演练

`pnpm test:workbench-extension` 使用脚手架在真实源码树中生成临时 `extension_probe`，直接编译 Rust、生成合同、检查 Svelte/TypeScript，运行生成的前端业务测试，再通过统一 playground 发现该类型。结束时恢复注册与生成合同，删除本次创建的六个文件。它比较已有生产文件的内容，排除机械生成 DTO 后，实际共享生产修改只有：

1. `crates/rambledesk-core/src/workbenches/registry.rs`
2. `apps/desktop/src/lib/workbench/definitions/registry.ts`

没有手工补 App、宿主、发布、数据库、HTTP/Tauri、输入写回或预览类型分支。生成的 Rust 两项测试与前端两项测试通过，Svelte/TypeScript 0 错误、0 警告，playground 自动发现 `extension_probe.json`。其 Result 与开发评分类型同为 `{score,note}`，包往返测试确认不会因形状重叠丢失类型身份。

脚手架工具另覆盖开发注册开关、dry-run、重复/保留名称、路径、拒绝覆盖和失败前置检查。准备脚本覆盖自动发现、开发用例默认排除、附件范围、不可变运行输入和未完成运行恢复约束。

## 保留的开发评分类型

`rating_review` 只在 Rust `workbench-fixtures` 和前端开发开关下注册。它的三个业务代码文件是 Rust 模块、definition 与 View；另有自己的业务/字段集成测试和 playground 材料。

| 验收点 | 实际证据 |
| --- | --- |
| 后端正常目录、schema 与请求 | storage 的 `rating_fixture_uses_real_discovery_save_recovery_cas_and_publication` 通过真实 service/repository 接口执行；不是预览 transport |
| 草稿保存、重开和 CAS | 同一测试使用 SQLite 与磁盘反馈包，重开恢复后检查评分、意见、正文与冲突 |
| 未完成意见可存、正文不能绕过评分 | 合法未完成 State 保存；补充正文不能完成无评分的请求；非法评分、超长意见、未知字段拒绝 |
| 发布、取消与历史保留 | service 正常发布与读取 manifest/result，未知版本保留原值且禁止发布；原类型取消与历史回归继续通过 |
| 新文本字段共享输入 | `rating_review/fieldIntegration.test.ts` 实际从 registry 解析意见字段，验证粘贴回执、附件去重/删除、语音来源与整理、材料变化和异步人工修改保护 |
| 普通和全屏宿主 | `App.ratingReview.test.ts` 通过 App、共享保存与发布控制器检查评分/意见/正文保存、卸载重开、全屏往返、左侧会话列表和只读历史 |
| 实际浏览器布局 | 在开发 Vite 的完整 App 预览中编辑评分与意见、打开全屏页签；1280 × 720 浏览器截图确认左侧会话列表可见，评分和意见保留。预览持久化仍是模拟，不替代上方真实后端测试 |
| 生产隔离 | 默认 Cargo 工作台目录回归通过；`VITE_WORKBENCH_FIXTURES=1` 的生产 Vite build 仍无 rating renderer manifest 项，86 个 JS chunk 不含评分业务字符串 |

## 现有工作台与生命周期

前端全套：272 文件通过、1 文件跳过；1882 项通过、1 项跳过。Svelte 检查为 0 错误、0 警告。Pi 22 项、dsh 28 项通过；准备及网页样例服务检查通过。

默认 Rust workspace（排除独立 target 的 desktop）全套通过；desktop 的 130 项单元/合同/事件/传输与二进制回归通过。开发 feature 的 core 工作台 16 项、storage 工作台 16 项通过，其中包含真实评分闭环。默认 workspace 全 targets 和开发 feature 的 core/storage Clippy 均零警告通过；格式、合同与模块边界检查通过。

终端回归涵盖真实 Windows ConPTY 的启动、停止、退出重开、多轮记录、最终输出和进程释放；HTTP 测试验证伪造客户端 stopped 状态无法绕过实际活跃 PTY。前端统一请求 controller 验证视图卸载后仍等待已接受按键，任务页签与原生控制台使用同一收尾入口，丢失会话保留历史，已关闭请求不能重新启动或写入。发布与 PTY 创建共享后端生命周期锁。

独立代码审查发现并修复了生成模板缺少草稿规则、重叠 Result 误解码、视图卸载期间会话丢失阻塞提交，以及新类型字段严格校验不一致。全套测试还修正了懒加载后测试等待业务视图的时序，未减少原行为断言。

## 人工与设备边界

本分支没有代填人类反馈，也没有把测试结果记成用户体验通过。实际麦克风、截图权限、原生浮窗和物理手机的人工体验尚未重新执行；Linux/macOS 的真实 PTY 设备行为本轮也未验证。现有 playground 六场景和开发评分材料保留这些检查，未体验的项应记录“未验证”。这些设备检查不影响普通类型只修改两个共享注册文件的扩展演练结论。

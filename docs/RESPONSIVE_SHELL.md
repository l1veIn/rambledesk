# Workbench 响应式 Shell

> 状态：2026-09-08 实现完成，等待真机人工验收。
> 范围：Browser Client（Web Access）在手机上的可用性；定位为 dogfooding，**不进入
> [WEB_ACCESS_SUPPORT_MATRIX.md](WEB_ACCESS_SUPPORT_MATRIX.md) 的支持承诺**。
> 术语源：[TERMINOLOGY.md](TERMINOLOGY.md)。

## 一、断点

| 模式 | 视口宽度 | 侧栏与请求列 | 拖动调整宽度 |
| --- | --- | --- | --- |
| `desktop` | ≥ 1024px | 双列，宽度可拖 | 有 |
| `tablet` | 768–1023px | 双列，按容器预算自动收窄 | 有 |
| `phone` | < 768px | 两个抽屉，正文占满整屏 | 无 |

断点定义只有一处：`apps/desktop/src/lib/workbench/shellMode.ts`（`PHONE_QUERY`、`TABLET_QUERY`、
`shellModeFor()`）。`WorkbenchShell.svelte` 在 `onMount` 里用 `matchMedia` 订阅变化，SSR 与首次渲染
默认 `desktop`。

## 二、行为矩阵

| 行为 | desktop / tablet | phone |
| --- | --- | --- |
| 侧栏 | 常驻列，`NavigationResizeHandle` 拖动或折叠成 56px | 左侧抽屉，宽 `min(85vw, 20rem)`，关闭时 `translateX(-100%)` |
| 请求列 | 常驻列，同上 | 左侧抽屉，与侧栏互斥 |
| 背板 | 无 | 抽屉打开时覆盖正文，点击关闭 |
| 关闭方式 | 折叠按钮 | 抽屉内的折叠按钮、背板点击、`Escape` |
| 打开方式 | 折叠按钮 | 标题栏左上角的导航按钮（替代品牌 logo）；请求列表用左下角浮动按钮 |
| 焦点 | 不变 | 抽屉打开时焦点移入抽屉；关闭的抽屉 `inert` |
| 正文 | 不变 | 抽屉打开时正文 `inert` |
| 标题栏左块 | 跟随侧栏宽度 | 固定 56px 的导航按钮 |
| 窗口边框 | 由 `environment` 决定，与视口宽度无关：桌面应用有圆角与描边，Browser Client 无（见第五节） | 同左 |

抽屉互斥由 `App.svelte` 的 `setHostRailCollapsed` / `setRequestRailCollapsed` 保证：打开一个会关闭另一个。

## 三、状态模型

折叠状态有两个层次，**这是本次改动的核心**：

| 状态 | 归属 | 持久化 | 含义 |
| --- | --- | --- | --- |
| `hostRailPreference`、`requestRailPreference` | `App.svelte` | 是（`uiPreferences`） | 桌面/平板下的折叠偏好 |
| `phoneHostRailOpen`、`phoneRequestRailOpen` | `App.svelte` | 否 | 手机抽屉的临时开合 |

有效折叠值 `hostSessionRailCollapsed = shellMode === 'phone' ? !phoneHostRailOpen : hostRailPreference`。
手机上关闭抽屉**不会**写回偏好，因此回到桌面宽度时侧栏恢复原来的宽度与折叠状态。Rust 侧不参与该状态。

组件契约：`HostSessionRail` 与 `RequestListPane` 新增可选 `onCollapsedChange`。父组件传入时由父组件
持有折叠状态（手机抽屉路径），否则保持原有 `bind:collapsed` 语义（dev 预览与其它调用方不受影响）。

## 四、触摸适配

| 项 | 处理 |
| --- | --- |
| `hover:` 粘滞 | Tailwind 4 的 `hover:` 已编译为 `@media (hover:hover)`，触摸设备不再触发，无需逐处改写 |
| hover 才显示的控件 | `HostSessionRail` 的置顶/归档/新建会话与展开箭头、`WorkspaceTabStrip` 的关闭按钮在 `pointer: coarse` 下常显 |
| 点击目标 | `Button` 组件的全部尺寸档位在 `pointer-coarse:` 下放大（`icon-xs` 24→32px、`icon-sm` 28→36px、`default` 32→40px 等），一处改动覆盖全应用 |
| 安全区 | `index.html` 增加 `viewport-fit=cover`、`interactive-widget=resizes-content`；`app.css` 在 < 768px 给 `body` 加 `env(safe-area-inset-*)` 内边距 |
| 浮动按钮 | 44×44px，位于安全区之内 |
| 拖动 | 手机上不渲染 `NavigationResizeHandle`（触摸拖动与滚动手势冲突） |
| Tab 条 | 标签宽度 = `clamp(112px, 可用宽度 / 标签数, 192px)`，到 112px 后横向滚动；宽屏与手机同一套逻辑 |
| Agent 输入框配置 | < 768px 时模型/思考强度/访问权限等会话选项收进一个入口按钮，点击弹出完整列表；宽屏保持内联选择器 |
| 减少动效 | 抽屉过渡遵循 `prefers-reduced-motion` |
| 浮动按钮 | 44×44px，位于安全区之内；请求列表按钮固定在左下角 |

## 五、Browser Client 的整页外观

Browser Client 是整页 Web 应用，不再复用桌面窗口的外观：

- `App.svelte` 的 `<main>` 在 `environment === 'browser'` 时去掉 `rounded-[16px] border shadow-sm`；
- `main.ts` 为 browser 入口追加 `body.web-mode`，`app.css` 用它去掉桌面窗口的 1px 发丝内边距；
- 手机上仍然保留 `env(safe-area-inset-*)` 内边距，避免内容落进刘海和 home indicator 区域。

`environment` 由 `createWorkbenchComposition()` 返回，桌面与浏览器各自传入，组件不嗅探全局变量。

## 六、已知限制

- **tablet 仍是双列**：768px 宽时两条 rail 会按 `fitNavigationWidths` 的预算收窄到约 192 + 200px，
  正文只剩约 376px。手机上因为容器宽度小于最小预算，双列会把正文压到 0，所以必须抽屉化；平板暂不处理。
- **标题栏 Tab 条**：宽度由 `lib/workspace/tabStripLayout.ts` 统一计算（可用宽度均分，夹在
  112–192px 之间），到最小宽度后才滚动。滚动容器隐藏滚动条，用边缘渐隐提示还有内容；桌面端鼠标滚轮
  会转成横向滚动，触摸端直接滑动。标题在收缩过程中被遮罩截断，触摸端关闭按钮常显（见第四节）。
- **抽屉没有手势关闭**：只有按钮、背板和 `Escape`。
- **Web Access 仍只绑定 loopback**：手机接入依赖用户自备的转发方式，本仓库不提供 LAN/TLS 承诺。

## 七、自动化覆盖

| 文件 | 覆盖 |
| --- | --- |
| `lib/workbench/shellMode.test.ts` | 断点映射、phone/tablet 区间不相交 |
| `lib/workbench/workbenchShellRender.test.ts` | 桌面双列、手机抽屉与背板、浮动按钮的出现条件、启动失败分支 |
| `lib/components/navigation/railResize.test.ts` | 既有宽度预算与折叠数学（未改动） |
| `lib/components/navigation/sessionRailRender.test.ts` | 侧栏渲染（未改动，回归通过） |
| `lib/mediaQuery.test.ts` | 媒体查询 store 的初始值、变更订阅与无 `matchMedia` 降级 |
| `lib/workspace/tabStripLayout.test.ts` | 均分宽度、最小宽度后转为滚动、未测量时的回退 |
| `lib/workspace/workspaceTabStripRender.test.ts` | 每个 tab 的测量宽度、滚动容器与边缘渐隐变量 |
| `lib/editor/MarkdownPreview.test.ts` | what-happened 风格的 markdown 渲染（标题/列表/粗体/代码块）与 bare 变体 |
| `lib/agents/configuration/sessionConfigurationControlsCompact.test.ts` | 手机端单一入口、弹层选项与选择回调；宽屏保持内联控件 |

## 八、人工验收

预览页（dev-only，与 `navigation-resize-preview.html` 同一模式）：

```bash
pnpm dev:web
# 打开 http://127.0.0.1:1420/workbench-shell-preview.html
```

用浏览器设备模拟或缩放窗口，逐项确认：

1. ≥ 1024px：双列、拖动改宽、折叠、刷新后宽度与折叠状态保留。
2. 768–1023px：双列收窄，正文仍可读。
3. < 768px：两列变成抽屉，正文占满；标题栏左上角变成导航按钮，左下角出现「打开请求列表」浮动按钮。
4. 打开一个抽屉时另一个关闭；背板点击、抽屉内折叠按钮、标题栏按钮、`Escape` 都能关闭。
5. 手机旋转屏幕后模式正确切换，且桌面宽度下的折叠偏好没有被手机操作覆盖。
6. Browser Client 整页无圆角、无描边；桌面应用仍然保留窗口圆角与描边。
7. < 768px 打开 Agent 会话：输入框下方只显示一个模型入口，点开后能改模型、思考强度、访问权限；
   打开多个 tab 时先变窄，缩到最小宽度后才横向滚动。

真机（用户手机通过既有方式访问 Web Access）至少确认：抽屉开合、请求切换后抽屉自动关闭、
虚拟键盘弹出时正文与输入区不被遮挡。

# RambleDesk shadcn 工作台重构

> 状态：实施中。
> 术语源：[TERMINOLOGY.md](TERMINOLOGY.md)。

这不是样式替换。目标是把 desktop 前端重构为稳定的人类反馈工作台，并用
shadcn-svelte 统一基础组件、主题 token、交互状态和目录边界。

## 信息架构

### Inbox

- 默认入口；
- 使用“宿主/会话 + 请求”的双栏导航；
- 第一栏按 Host Profile 分组并展开宿主会话，用于筛选；
- 第二栏显示当前宿主或会话范围内的全部 requests；
- 显示标题、时间和 request 状态；
- 选择请求进入 Request Workspace；
- 终态请求留在同一 requests 列表中，不按终态拆分分页；
- 不展示全局 transport 指示器。

### Request Workspace

- 请求说明和操作清单；
- Ramble 输入、富文本编辑、截图、录音和附件；
- 草稿保存与冲突恢复；
- 提交、取消和完成后的只读反馈包；
- 当前 request 的 host/source 信息只作为上下文，不成为导航对象。

### Resume Prompt

- 仅在需要 continuation 的终态请求上出现；
- 显示目标 Host Profile 和可复制提示；
- 提供复制和关闭动作；
- Pi 原生等待流程不显示该入口。

### Settings / Adapters

- General：语言、主题；
- Adapters：Generic MCP Adapter、Pi Native Adapter 和未来适配器；
- 每个适配器独立显示安装状态、配置动作、说明和错误；
- 适配器页面不承担全局 transport 监控。

### Tray

- 打开 Inbox；
- 显示未处理数量；
- 快速进入当前 Request Workspace；
- 打开 Settings / Adapters；
- 退出应用。

## 目标目录

```text
apps/desktop/src/
├── app.css
├── components.json
├── lib/
│   ├── components/
│   │   ├── ui/                  # shadcn-svelte primitives
│   │   └── navigation/          # Host/Session + Request 双栏导航
│   ├── workbench/               # Request Workspace、Resume、Ramble
│   ├── screen-capture/          # 截图编辑器专用组件和样式
│   ├── generated/
│   ├── SettingsPanel.svelte
│   ├── AppTitlebar.svelte
│   └── utils.ts
├── App.svelte
├── ScreenshotOverlay.svelte
└── ScrollCaptureController.svelte
```

规则：

- `components/ui` 只放 shadcn primitives 和薄封装；
- `App.svelte` 装配查询、command、事件订阅和页面级 view model；
- 业务组件只接收明确 props/events，不直接散落 Tauri invoke；
- 原生截图覆盖层保留专用结构，不强行卡片化；
- Rust DTO 继续从 `generated/` 导入。

## 组件基线

使用：

- `Button`：明确命令，图标优先；
- `Badge`：离散状态；
- `Tabs`：Settings 顶层视图；
- `Dialog`：Settings 与 Resume Prompt；
- `Tooltip`：图标按钮；
- `Select`：语言、主题和适配器选项；
- `Switch`：通知等二元设置；
- `Separator`、`ScrollArea`、`Skeleton`、`Alert`；
- `DropdownMenu`：tray 投影之外的紧凑命令集。

避免：

- 页面区块全部做成浮动卡片；
- card 内再嵌套 card；
- 用圆角文字块代替熟悉图标；
- 装饰性大标题、营销 hero 或渐变背景；
- 在 UI 中解释快捷键或产品功能；
- 让 transport 状态占据全局导航。

## 视觉基线

- 工作台采用安静、紧凑、可扫描的桌面工具密度；
- 圆角不超过 8px，窗口本身的原生圆角除外；
- 主色、成功、警告、危险使用独立语义 token；
- 亮色、暗色、跟随系统使用同一 token 集；
- 不用 viewport width 缩放字体；
- 固定工具条、按钮、计数器和捕获工具设置稳定尺寸；
- 980、1180、1320 px 均不得横向溢出或文字遮挡；
- 中文与英文必须使用同一布局约束。

## 交互合同

- Settings 和 Resume Prompt 支持 Escape、背景关闭和焦点回收；
- 图标按钮都有可访问名称和 tooltip；
- Inbox 键盘选择后可进入 Request Workspace；
- Request 切换前先完成 Ramble 收尾并保存草稿；
- terminal request 只读；
- 原生事件只触发重新查询，不直接覆盖事实状态；
- 浏览器预览中的 native hooks 必须安全降级。

## 实施顺序

1. 接入 Tailwind v4、shadcn-svelte、`components.json` 和 `cn()`；
2. 建立语义 token 与 Button/Dialog/Tabs/Tooltip 等 primitives；
3. 拆分 App shell、Inbox 和 Request Workspace；
4. 重构 Settings 为 General / Adapters；
5. 重构 Resume Prompt 与 continuation 展示；
6. 统一截图、浮动控制台和附件工具条；
7. 删除旧样式目录、旧组件路径和无引用 CSS；
8. 运行完整自动化、响应式截图和原生人工验收。

迁移过程中每个业务区域完成后即删除对应旧 CSS，不保留长期双轨样式。

## 自动化门禁

```bash
pnpm check
pnpm test
pnpm build:web
pnpm contracts:check
```

浏览器视觉检查：

- 980、1180、1320 px；
- 亮色与暗色；
- 中文与英文；
- Inbox 空态、列表态、选中态；
- Request Workspace waiting/in_progress/completed/cancelled；
- Settings General/Adapters；
- Resume Prompt；
- 无横向溢出、重叠、空白画布或错误 native hook。

原生人工检查：

- macOS/Windows 自绘 titlebar 与窗口拖动；
- tray 入口与未处理计数；
- Settings 和 Resume Prompt 焦点；
- 截图覆盖层、DPI、权限和 Escape；
- Ramble 录音与截图并行；
- completed package 打开与复制 continuation。

## 收口标准

- `styles/` 旧目录删除；
- 全局 transport 指示器及其文案删除；
- Settings 不再围绕某一种 transport 组织；
- App shell 不持有 request 事实状态副本；
- shadcn primitives 有稳定目录与统一变体；
- 所有自动化门禁通过；
- 剩余原生视觉点明确交给人工验收。

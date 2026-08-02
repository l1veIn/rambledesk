# 前端 shadcn-svelte 全面迁移方案（计划文档）

> 状态：**已决策，未实施**。本迭代只落盘迁移方案；实施从后续迭代开始。
> 决策来源：2026-08-02 dogfood 反馈（操作者提议引入 shadcn，经确认走全面迁移路线）。
> 相关：`docs/DEVELOPMENT.md`、`apps/desktop/src/app.css`、`apps/desktop/src/styles/*`。

---

## 1. 目标

把 RambleDesk 桌面前端从「手写 CSS + CSS 变量设计系统」全面迁移到
**shadcn-svelte + Tailwind CSS**，用 shadcn 组件与主题 token 统一全部 UI。

- 迁移后不允许新旧两套样式长期共存：**一次迁移、一次收口**。
- 行为回归门槛：以 `docs/DOGFOODING.md` 的既有验收清单为准，逐组件过一遍。
- 迁移期间以迭代为界，每次迭代结束必须跑完 `README.md` 的 Verification 全部门禁。

---

## 2. 现状盘点

### 2.1 设计 token（`apps/desktop/src/app.css`）

`:root` 与 `:root[data-theme="dark"]` 两套，共 21 个变量：

| 变量 | 亮色示例 | 用途 |
| --- | --- | --- |
| `--ink` / `--ink-soft` / `--ink-faint` | `#20334b` / `#60738a` / `#8b9aaa` | 文本三级 |
| `--surface` / `--surface-raised` / `--surface-tint` | `#f7f9fc` / `#ffffff` / `#f0f5fa` | 面板三级 |
| `--line` / `--line-soft` | `#c8d5e3` / `#dbe4ee` | 描边两级 |
| `--blue` / `--blue-strong` / `--blue-soft` | `#4f8fd3` / `#2775ca` / `#e8f2fd` | 主色三级 |
| `--cyan` / `--cyan-soft` | `#32aaa4` / `#e7f7f5` | 成功/链接辅助 |
| `--amber` / `--danger` | `#e99725` / `#c94a52` | 警示/错误 |
| `--shadow` | `0 16px 48px rgb(42 70 101 / 8%)` | 弹层阴影 |
| `--app-background` | `#edf2f7` | 窗口底 |
| `--glass` | `rgb(247 249 252 / 92%)` | 毛玻璃 |
| `--hover` / `--editor-paper` | `#f5faff` / `#fff` | 悬停/编辑器纸面 |

主题切换：`<html data-theme="dark">`，由 `preferences.ts` 的 `themePreference` 驱动。

### 2.2 组件清单（23 个 .svelte + 7 个 CSS 文件，约 1900 行样式）

| 组件 | 现状 | 迁移目标组件 |
| --- | --- | --- |
| `App.svelte` | 窗口壳、布局、路由态 | 壳 + `Dialog`/`Sheet` 编排 |
| `lib/AppTitlebar.svelte` | 自绘标题栏、窗口按钮、控制台开关 | 保留自绘或迁 `Titlebar` 方案 |
| `lib/SettingsPanel.svelte` | 自绘设置弹窗（含适配器区） | `Dialog` + `Tabs` + `Select` |
| `lib/RichFeedbackEditor.svelte` | Tiptap 富文本编辑器 | 编辑器保留 Tiptap，外壳组件化 |
| `lib/workbench/*`（11 个） | 收件箱/工作台/命令轨/交付卡片等 | `Card`/`Button`/`Tabs`/`Badge` 等 |
| `RambleConsole.svelte` | 图标控制台 | `Toolbar` + 自定义浮层 |
| `ScreenshotOverlay.svelte` / `PinnedCapture.svelte` / `ScrollCaptureController.svelte` / `lib/screen-capture/CaptureToolbar.svelte` | 截图覆盖层 | 覆盖层自绘为主，工具按钮组件化 |
| `styles/*.css`（7 个） | 手写样式 | 全部删除，token 并入 Tailwind 主题 |

### 2.3 现有关键交互约束（迁移必须保持）

- 窗口 `decorations:false + transparent`，`.shell` 16px 圆角，遮罩需与窗口圆角对齐。
- 设置弹窗：背景点击关闭、ESC 关闭、`aria-modal`。
- 恢复提示弹窗 `ResumePromptDialog`：`rambledesk://resume-prompt` 事件驱动。
- 截图覆盖层：跨 Webview 全屏、指针框选、区域高亮。
- 中英文 i18n：文案一律走 `i18n.ts` 的 `t()`，组件内不得硬编码。
- 亮/暗两套主题 + 跟随系统。

---

## 3. 目标架构

```
apps/desktop/src/
  components.json            # shadcn-svelte 配置
  app.css                    # 删除；token 迁入 Tailwind @theme
  lib/components/ui/*        # shadcn 组件（bits-ui + tw-animate-css）
  lib/components/**/*.svelte # 业务组件
  lib/utils.ts               # cn()（tailwind-merge + clsx）
```

### 3.1 依赖

| 包 | 用途 |
| --- | --- |
| `tailwindcss` + `@tailwindcss/vite` | Tailwind v4（CSS-first 配置，无 tailwind.config 亦可） |
| `tailwindcss-animate` / `tw-animate-css` | 动效 |
| `bits-ui` | shadcn-svelte 底层（Dialog/Tabs/Select…） |
| `lucide-svelte` | 图标（已在使用，保留） |
| `clsx` + `tailwind-merge` | `cn()` |
| `melt-ui` | bits-ui 依赖，间接引入 |

### 3.2 Token 映射表（迁移时的对照基准）

| 现 CSS 变量 | shadcn 语义变量（hsl） | 备注 |
| --- | --- | --- |
| `--ink` | `--foreground` | 主文本 |
| `--ink-soft` | `--muted-foreground` | 次级文本 |
| `--ink-faint` | `--muted-foreground`（降透明度） | 或 `--muted-foreground/70` |
| `--surface` | `--background` | 页面底 |
| `--surface-raised` | `--card` / `--popover` | 卡片/浮层 |
| `--surface-tint` | `--muted` / `--secondary` | 次级面板 |
| `--line` | `--border` | 描边 |
| `--line-soft` | `--border`（透明度 ~60%） | 次级描边 |
| `--blue` | `--primary`（中色调） | 主色 |
| `--blue-strong` | `--primary`（hover 态） | 主色按压 |
| `--blue-soft` | `--primary-soft`（自定） | 主色底 |
| `--cyan` | `--success`（自定） | 成功/激活 |
| `--cyan-soft` | `--success-soft`（自定） | 成功底 |
| `--amber` | `--warning`（自定） | 警示 |
| `--danger` | `--destructive` | 错误 |
| `--shadow` | 保留为 shadow token | 弹层阴影 |
| `--app-background` | `--background` 微差 | 窗口底 |
| `--glass` | `--glass`（自定） | 毛玻璃 |
| `--hover` | `--accent` | 悬停底 |
| `--editor-paper` | `--card` | 编辑器纸面 |

> 注意：shadcn 用 `oklch`/`hsl` 色值，映射时**以肉眼一致为准**，不要机械抄 HEX。
> 迁移后运行 `pnpm check` + 亮暗两套主题的视觉 dogfood 各一轮。

---

## 4. 迁移步骤（建议顺序，每步一个迭代）

1. **基础设施**：装 Tailwind v4 + shadcn-svelte init；建立 `components.json`、`lib/utils.ts`；
   把 `app.css` 的 token 换算为 shadcn CSS 变量（含 dark 主题）；删掉旧 `app.css` 变量。
   → 门禁：`pnpm build:web`、`pnpm check`、亮暗切换无跳变。
2. **通用原子组件替换**：`Button` / `Badge` / `Card` / `Tabs` / `Select` / `Switch` / `Dialog` /
   `Tooltip` / `Skeleton` / `Separator`。逐文件替换 `styles/*.css` 中的对应类。
3. **SettingsPanel → shadcn `Dialog` + `Tabs` + `Select` + 自定义 Adapter 卡片**：
   保持 ESC/背板点击/焦点管理；适配器区保留现有 `<details>` 交互语义（或换 `Collapsible`）。
4. **Workbench 系列**：InboxPanel / WorkspacePanel / TaskBriefPanel / FeedbackEditorPanel /
   AttachmentsCard / CaptureToolsCard / DeliveryCard / RambelleStatusCard / RamblePanel /
   CommandRail / WorkspaceHeader。
5. **浮层与控制台**：RambleConsole、ResumePromptDialog、截图覆盖层（自绘为主，仅工具条组件化）。
6. **收口**：删除 `styles/` 全部文件；`grep` 全局确认无 `var(--...)` 残留；
   跑全量 Verification + 一轮完整 dogfood（含中英文、亮暗、1320/1180/980 视口）。

### 4.1 每步验收清单（通用）

- [ ] `pnpm check` / `pnpm test` / `pnpm build:web` 通过。
- [ ] 该组件亮色、暗色、跟随系统三态视觉一致。
- [ ] 中文、英文文案完整（i18n 键无硬编码）。
- [ ] 交互行为与迁移前一致（焦点、键盘、点击区域）。
- [ ] 无横向溢出（1320/1180/980 三档）。

---

## 5. 风险与约束

1. **窗口圆角/透明**：Tauri 窗口 `transparent:true`，body 背景透明；
   任何覆盖层都要显式处理圆角对齐，迁移 Dialog 时不得回归（参见本轮
   `settings-backdrop` 底部圆角 16px 的修复）。
2. **Tiptap 编辑器**：`RichFeedbackEditor` 是 Tiptap 实例，组件化外壳可换，核心编辑能力不动。
3. **截图覆盖层**：全屏跨 Webview、指针事件、区域捕获，是桌面特有路径；浏览器 `dev:web`
   下需按现状降级（native hooks 已守卫），迁移不得引入浏览器端未守卫的依赖。
4. **双样式并存期**：步骤 1-5 期间新旧样式并存，可能出现视觉杂音——**每个迁移迭代收尾必须
   清理该组件对应的旧 CSS 文件段落**，不允许"先留着"。
5. **主题 token 语义**：`--cyan` 同时承担"成功"与"激活/录音中"两种语义，映射时拆成
   `--success` 与 `--active` 两个 token，避免一处改全部变。
6. **i18n 键**：迁移中如改文案，zh/en 两个表必须同步；新增 UI 文案一律先加键。
7. **性能**：Tailwind v4 JIT 与 vite 集成正常；注意 `build:web` 产物体积可接受（当前无体积门禁，记录基线即可）。

---

## 6. 明确不做（Out of scope）

- 本轮（2026-08-02）**不实施迁移**，只落盘本方案。
- 不引入 shadcn 的 React 生态组件；仅用 shadcn-svelte（bits-ui 底层）。
- 不重做产品信息架构；迁移纯 UI 层。
- 不迁移 `crates/*` 与 MCP/协议层。

---

## 7. 决策记录

- 2026-08-02：操作者在 dogfood 反馈中提出「模态框重做，是否引入 shadcn」；
  经结构化确认：**全面迁移，但本轮仅写迁移文档**；设置弹窗本轮只保留圆角修复、不做大改版。
- 备选方案（未选）：保持原生 CSS 按需打磨；仅新组件用 shadcn 与旧样式共存。

# 响应式、键盘与焦点验收

> 2026-09-09 至 09-10；质量计划 Q15 的组件合同与分批浏览器视口证据，包含 build5 的附件预览。
> 对象为 rc3 标签之上的本轮候选，最终代码提交为 `27b50f1` 与 `5d57526`；各批次受测产物分别记录，修复后的行为不能算作 rc3 原有能力。
> jsdom、真实浏览器 CSS 视口和物理设备结果分别登记，不能相互替代。

## 发现与处理

| 场景 | 修复前可复现结果 | 当前合同与入口 |
| --- | --- | --- |
| 手机侧栏 Escape 关闭 | 焦点留在即将隐藏的 drawer | [WorkbenchShell](../../apps/desktop/src/lib/workbench/WorkbenchShell.svelte) 归还 opener；关闭后 workspace 解除 inert |
| 请求抽屉内部关闭、遮罩关闭 | FAB 曾被卸载；旧节点无法接收焦点 | Shell 归还新建的 request opener；pointer 未先聚焦按钮也有 fallback |
| 侧栏 Search Dialog 中按 Escape | 内层搜索框与外层导航同时关闭 | 尊重内层事件的 `defaultPrevented`；第一次只关搜索框，第二次关闭 drawer |
| 标题栏在 drawer 打开时执行其他动作 | 需要避免关闭后的异步 focus 抢回 | 已主动移到另一标题栏动作的焦点保留；销毁后的延迟 focus 不执行 |
| pointer 打开侧栏但浏览器未聚焦按钮 | 没有可用的 opener 引用 | [AppTitlebar](../../apps/desktop/src/lib/shell/AppTitlebar.svelte) 的 `aria-controls` 连接真实侧栏；Shell 依此归还 |
| session 列表刷新中 | pointer 被阻止，键盘仍可访问旧行 | [HostSessionRail](../../apps/desktop/src/lib/components/navigation/HostSessionRail.svelte) 列表内容设为 inert；搜索和收起等控制保留，与 RequestListPane 一致 |
| pending / disabled 的页签收到 Delete 或中键 | 绕过已禁用的关闭按钮，仍调用 onClose | [WorkspaceTabStrip](../../apps/desktop/src/lib/workspace/WorkspaceTabStrip.svelte) 在公共关闭入口检查同一锁，键盘和 pointer 一致 |
| 关闭页签的触控命中范围 | 原始按钮为 16px，统一 Button 的粗指针规则未覆盖它 | 普通指针按钮为 24px、粗指针为 32px；维持小图标并给标签留白，真实命中范围待浏览器测量 |

没有更换导航、页签或设置的交互模型。现有 drawer 保留可用的标题栏，workspace 和关闭的 rails
由 inert 排除；它并非全屏模态对话框。因此本次没有声称 `aria-modal=true`，也没有把焦点强制困在
drawer 内、阻断标题栏切页。真正的 Search Dialog 继续由现有 Dialog primitive 管理模态和焦点。
若产品以后决定 drawer 应是全模态，需要同时处理标题栏、外部内容与焦点循环，不能只添加一个
Tab 拦截器。模态合同参见 [WAI-ARIA Dialog Pattern](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/)。

尺寸判断不采用“低于 44px 一律失败”。WCAG 2.2 AA 的最小目标是 24×24 CSS px 或满足列出的
例外；44×44 属于增强级 AAA。这里保留现有桌面密度，只处理明显狭小的叠加关闭按钮；较大的
Button 粗指针命中区、44px 请求 FAB 不做重复改造。本记录不构成整页无障碍符合性认证。
依据：[Target Size Minimum](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum)、
[Target Size Enhanced](https://www.w3.org/WAI/WCAG22/Understanding/target-size-enhanced)。

## 组件与合同验证

2026-09-09 23:45（Asia/Shanghai），下列 **7 个文件、32 条测试通过**；随后 `pnpm --dir apps/desktop check`
为 0 errors / 0 warnings，`git diff --check` 通过。

```sh
pnpm --dir apps/desktop test \
  src/lib/workbench/workbenchShellInteraction.test.ts \
  src/lib/workbench/workbenchShellRender.test.ts \
  src/lib/workspace/workspaceTabStripKeyboard.test.ts \
  src/lib/workspace/workspaceTabStripRender.test.ts \
  src/lib/workspace/workspaceTabStripScroll.test.ts \
  src/lib/components/navigation/sessionRailRender.test.ts \
  src/lib/shell/titlebarInteractions.test.ts
```

- [workbenchShellInteraction.test.ts](../../apps/desktop/src/lib/workbench/workbenchShellInteraction.test.ts)
  挂载真实 AppTitlebar、Shell、HostSessionRail、RequestListPane 及搜索 Dialog，验证上表的焦点归还、
  内层 Escape、外部焦点保留与刷新 inert 合同。仅补 jsdom 缺少的 media query、ResizeObserver 和动画 API。
- [workspaceTabStripKeyboard.test.ts](../../apps/desktop/src/lib/workspace/workspaceTabStripKeyboard.test.ts)
  挂载真实页签组件与 DnD 层；End/箭头只移动焦点、Enter 接受目标、Delete 关闭后归还到活动页签。
  disabled 与 pending 两种情况下，Delete 和中键不再触发关闭。
- 原有 SSR、滚动、session 层级与原生标题栏意图测试继续通过；原生窗口拖动本身仍需宿主验证。

上述焦点回归和 tab 锁回归均先在原实现中失败，再由生产修复通过。inert 测试验证传给 DOM 的语义
属性；jsdom 不模拟浏览器完整 Tab 导航、inert hit-testing、布局或触摸，因此不能把它当作真机证据。

## 保留与待补的浏览器验收

以下为组件回归后列出的待补范围；已完成的具体动作见后续日期记录，其余仍保持待验。

- RequestListPane 已有刷新 inert、视口限宽/限高的 Filter Popover 与粗指针 Button，不新增另一层
  弹层/按钮抽象。HostSessionRail 的行内操作已有粗指针可见性和右侧预留；实际间距仍用布局测量判断。
- SettingsPanel 保留单一 sections 元数据和垂直键盘方向。发现导航列在短窗口没有独立滚动；修复
  已将导航列表设为可滚动、品牌与按钮不收缩，并从同一元数据提供图标 title；本轮组件统计不包含设置修复。
- 浏览器需补：窄宽与短高下真实 Tab 顺序，drawer 关闭后 activeElement，Search/Filter 的逐层 Escape，
  页签关闭实际 bounding box 和邻接点击，设置底部栏目可滚动到达，横向页签滚动和缩放后的裁切。
- 手机软键盘、真实触屏、Safari/WebView 与原生桌面的宿主差异保持独立验收状态；此处不填写通过。

## 2026-09-10 浏览器视口复验

此前固定的 production frontend、真实 HTTP fixture、Codex Chromium 152；这里只是浏览器 CSS 视口，未启用手机触摸或软键盘模拟。

- **390×844**：drawer 打开后焦点进入 `host-navigation-pane`；Escape 后 `activeElement` 回到 `Open sidebar`。`documentElement.scrollWidth` 与 body 均为 390，无页面横向溢出。
- **844×390**：设置采用纵向栏目导航与独立内容滚动，实际截图中当前栏目和内容可见；导航区域高 278px。
- **844×260**：纵向栏目导航 `clientHeight=136`、`scrollHeight=156`，可滚动 20px；实际点击最后一项 About，`aria-selected=true`，按钮落在 y=222–248、视口以内，内容正确切换；页面 scrollWidth=844。
- 测量后已恢复默认视口。嵌套弹层 Escape、键盘锁和 coarse pointer 32px 仍分别由组件／CSS 合同证明；未把这些变成真实触控通过。真实 Chrome、Safari、原生应用的核心操作见 [实机记录](NATIVE_BROWSER_ACCEPTANCE.md)。
- 默认桌面视口下只读检查四个实际页签关闭按钮，bounding box 均为 **24×24 CSS px**。这补足普通指针命中尺寸；coarse pointer 的 32px 规则仍不等同手机上的实际点击验收。

## 2026-09-10 build5：Native Web Access 的移动尺寸复验

本轮由原生 RambleDesk Quality **build5** 在端口 `55903` 启动实际 loopback Web Access，IAB 浏览器
访问该服务并调整 CSS 视口。Web Access 显示 Running，本次启动未出现 Keychain 授权提示。
这与此前独立 HTTP fixture 分开记录，也不代表物理手机、Safari 或正式签名安装包已经通过。

| 实际操作 | 观察结果 |
| --- | --- |
| **390×844** 打开工作台 | 页面宽度为 390，无全页横向溢出 |
| 打开侧栏 drawer，再打开 Search，连续按两次 Escape | 第一次只关闭 Search，焦点归还 Search button，drawer 保留；第二次关闭 drawer，焦点回到 Open sidebar |
| 打开请求 drawer，再打开 Filter，连续按两次 Escape | 第一次 Filter 的 `aria-expanded=false`，drawer 保留，焦点回到 Filter requests；第二次关闭 drawer，焦点回到 Open request list |
| 在可见请求 drawer 中选择 03 附件请求 | 进入目标请求后 drawer 自动收起 |
| Files 触发真实 file chooser，再用 `setFiles` 选择仓库 [64x64.png](../../apps/desktop/src-tauri/icons/64x64.png) | Saved 从 r3 到 r5；图片同时出现在 Feedback Draft 和附件列表 |
| 对该图片打开 Preview | 实际 modal 内两个 img 均 `complete=true`，自然宽高均为 64×64，图片内容已解码显示 |
| 关闭图片 Preview | Close 成功；未充分观察焦点归还，因此该焦点动作仍待验 |
| **844×260** 打开 Settings 并滚动到 About | tablist 的 `clientHeight=136`、`scrollHeight=156`；实际 `scrollTop=20` 后 About 位于 y=222–248。点击后 About 被选中并显示相应页面 |

这些设置与 drawer 结果包含实际交互后的 UI 检查。个别自动化 click 首次只把目标滚动到位，未激活；
检查当前界面后通过可访问性入口完成操作，不把没有触发的工具动作登记为产品失败。

本轮已补足上述 Search/Filter 逐层 Escape、drawer 焦点归还、选择后收起、短窗口设置可达与图片预览。
尚未证明完整 Tab 顺序、所有相邻命中区域、图片 modal 关闭后焦点和全部缩放裁切；真实手机仍无设备，
软键盘、触控、物理旋转和安全区继续待验。

同一 build5 的独立 Chrome/Safari 断线重新认证路线曾发现保存停留在 Saving，作为历史失败保留。
定向修复后的 build6 已完成两端离线草稿保存恢复、交换顺序的同一草稿 CAS 和 token 轮换实测，
具体操作与边界见 [原生与浏览器记录](NATIVE_BROWSER_ACCEPTANCE.md)。这些独立浏览器结果与本节
IAB 尺寸结果分开登记，物理手机的软键盘、触控、旋转和安全区仍待验。

# 宣传页设计与素材

2026-09-10 升级以「把注意力，留给判断」为主题，沿用 README 的产品叙事：Agent 写清体验单，人直接体验、口述和编辑，再把不可变反馈包交回 Agent。网页让访客理解产品、看见反馈流程并找到下载入口。

## 视觉与交互意图

品牌继承 RepoChan 的 Rambelle：银白长发、冰蓝眼睛、银白制服与空心六边形。她在首屏递交透明档案板，结尾收起档案；动作对应请求与反馈的交付。浅色纸面、玻璃边缘和档案索引来自既有角色世界，信息层级遵循清楚、克制、诚实的内容设计原则。

全页按「明亮首屏 → 体验单 → 深色反馈示例 → 接入步骤 → 浅色收尾」展开。只用一个深色主段突出反馈过程。标题、状态、按钮、示例与结果均为真实 HTML，生产插画不承载产品信息。

- **导航与首屏：** 大字建立价值，右侧近景角色建立身份；导航离开首屏后增加底色。入场动效一次完成，不循环争夺注意力。
- **体验单：** 并列呈现「刚才发生了什么」与「请你体验或确认什么」，以具体设置页案例连接后续示例。
- **可点击的产品模型：** 深色段中间是一扇按真实工作台排布的应用窗口：标题栏标签页、项目与会话侧栏、请求列表、任务简报与反馈正文、右侧 Ramble 命令栏。访客可以切换项目、会话与请求，收放任务简报，点一条「需要体验」触发 `@Action` 分组，开始/停止记录写入待整理语音段，用「整理」清理它，附加示意截图，直接在正文里改字，然后用「提交反馈」看到反馈包与投递状态（排队中 → 投递中 → 已送达），或取消请求。会话内容、状态与结果全部是脚本化演示数据；窗口上方与模型下方都写明「交互示例 · 演示数据」，不录音、不连接 Agent、不冒充录屏，旧版真实 GIF 仍以链接形式保留。
- **开始使用与收尾：** 内容按阅读顺序轻量显现，回到平静的下载入口；没有滚动劫持或自动播放。
- **可访问性：** 主叙事无需 hydration；增强失效仍可阅读，尊重 reduced motion，交互支持键盘和可见焦点。

桌面内容与插画共享居中的 1664px 上限，深色反馈段使用 1320px 内层范围——正好是桌面窗口的最小尺寸（`tauri.conf.json` 的 `minWidth/minHeight` = 1320 × 840），产品模型按这个比例绘制：满宽时就是一扇真实窗口，窄于 1320px 时整体等比缩小，容器宽度低于 1100px 才收起侧栏并改为纵向堆叠（`ProductMock.svelte` 的 container query）。首屏按原画比例计算高度，宽屏两侧融合背景，避免 4K 下固定高度造成大幅裁切。960px 以下切换独立手机构图，图片放在正文下方；实际断点与样式以 [global.css](../../web/src/styles/global.css) 为准。

## 生产资产

生成工具为 `repochan image gen`，配置与服务回传模型为 `gpt-image-2.5-sunburst`。共同人物参考是 `ord-foundation-001` / `v2026-07-31T10-59-43-937Z`。这些是连续环境与角色复合插画，没有从整页设计稿截出产品 UI。

| 消费文件 | 尺寸 | 订单 / 不可变结果版本 |
| --- | --- | --- |
| [hero-desktop.webp](../../web/public/assets/refresh/hero-desktop.webp) | 2560 × 1600 | `ord-web-refresh-production-hero-desktop-20260910-001` / `v2026-09-10T09-33-23-205Z` |
| [hero-mobile.webp](../../web/public/assets/refresh/hero-mobile.webp) | 1024 × 1024 | `ord-web-refresh-production-hero-mobile-20260910-001` / `v2026-09-10T09-35-21-347Z` |
| [closing.webp](../../web/public/assets/refresh/closing.webp) | 1536 × 1024 | `ord-web-refresh-production-closing-20260910-001` / `v2026-09-10T09-37-14-801Z` |

产品模型另用四份桌面端自带的真实素材（直接复制，未重新生成，也没有二次压缩）：`rambledesk-app-icon.webp` 显示标题栏品牌，`rambelle-idle.webp`、`rambelle-recording.webp`、`rambelle-archived.webp` 是 Rambelle 状态卡在待命、记录中与反馈包已封存三种状态下的立绘。它们与 `apps/desktop/src/assets/` 中的同名文件逐字节相同；改动任一侧都要同步另一侧。

SHA-256：

```text
hero-desktop.webp          21461b47b73fba79d27a1a97621af1b03b5a5078ca9913c86508affeb1c1e3b3
hero-mobile.webp           aed8fa376d836259b38abb8638b3bd6e5ce6512ce307144098f73e5a325cf118
closing.webp               e36d1776b19045977c64cce8d7d7edcadedaf517476439871a40d3d0e45da4f7
rambledesk-app-icon.webp   c0b74b764e32d6a00a97b327566c6ee8b77a5ef88989e2cf15fff7081c50b7cd
rambelle-idle.webp         9fffb56b9ff99d029d27c5144a8fa838c9cb0e20f8a69207e15d77ebab6738bb
rambelle-recording.webp    ee5f86a04e53fa4519114a264e1bae99904a7ddb59e6c94889740189c76b6ec4
rambelle-archived.webp     2b10e889b10046ab9cb132456c4c1a2d28fdd76638a0077318d4d351da3f65eb
```

[几何品牌标记](../../web/public/assets/refresh/rambledesk-mark.svg) 为手写 SVG，只用于网页导航、页脚与 favicon。README 的 Q 版收尾图另见[品牌图片来源](../social/README.md)。完整提示词与生成原图保存在 RepoChan 对应订单版本；设计迭代原稿和截图保留在设计者本机的 `.local-artifacts/marketing-refresh-20260910/`，不作为运行依赖。

## 实现验证与维护

设计方向已获用户确认。2026-09-10 实现检查覆盖中英文、360–3840 CSS px 的 24 个响应式案例，包括断点两侧、4K 居中、首屏图框与横向溢出；静态检查零错误/警告，构建输出两种语言。视口测试不等于物理手机或不同显示器缩放下的人类审美验收。

文案与版本入口、示例数据、开发和部署命令统一见 [web/README.md](../../web/README.md)。可点击的产品模型由 [ProductMock.svelte](../../web/src/components/ProductMock.svelte)（状态与三条导航栏）、[MockWorkspace.svelte](../../web/src/components/MockWorkspace.svelte)（任务简报、反馈正文、Agent 与设置视图）、[MockCommandRail.svelte](../../web/src/components/MockCommandRail.svelte)（Ramble、上下文、附件、反馈包、Rambelle 状态卡）、[MockCaptureThumb.svelte](../../web/src/components/MockCaptureThumb.svelte)（示意截图）与 [product-mock.ts](../../web/src/content/product-mock.ts)（脚本数据与双语文案）承担：改文案或状态要同时检查中英文、键盘操作和 reduced motion，并对照 `apps/desktop/src/lib` 里的真实组件，界面不随口径漂移。新产品录屏可以替换这套模型，但应保留手动播放、文字说明和准确的反馈交付状态。

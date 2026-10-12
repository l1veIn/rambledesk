# 宣传页设计与素材

2026-10-12 更新将主页叙事同步为「生成式 UI 在工作场景的应用」：从熟悉的问答出发，展示文稿审阅、图片标注、头脑风暴和自由反馈，让访客看见工作台如何把意见与对应材料交回 Agent。下载入口更新为 0.5.0；新版 Markdown 审阅和头脑风暴标为开发预览。

沿用 2026-09-10 制作的品牌插画与浅色纸面风格，用真实界面录制替换当时的脚本产品模型。以下生产插画来源仍适用于当前页面。

## 视觉与交互意图

品牌继承 RepoChan 的 Rambelle：银白长发、冰蓝眼睛、银白制服与空心六边形。她在首屏递交透明档案板，结尾收起档案；动作对应请求与反馈的交付。浅色纸面、玻璃边缘和档案索引来自既有角色世界，信息层级遵循清楚、克制、诚实的内容设计原则。

全页按「明亮首屏 → 工作台协作流程 → 深色工作台演示与目录 → 接入步骤 → 下载」展开。只用一个深色主段突出工作台。标题、说明与播放控件均为真实 HTML，生产插画不承载产品信息。

- **导航与首屏：** 大字建立价值，右侧近景角色建立身份；导航离开首屏后增加底色。入场动效一次完成，不循环争夺注意力。
- **工作台流程：** 从问答引入任务材料与不同交互方式，说明怎样将人的判断连同材料交回 Agent。
- **真实界面演示：** 中英文使用对应语言的四段六秒录制。页签切换工作台，每次展示完整画面；默认静态 WebP 封面，用户点击后才加载 MP4。支持键盘切换与暂停，不自动播放或循环。材料为演示数据，不将录屏描述成真实 Agent 会话。视频与封面来源见[媒体说明](../../web/public/assets/workbenches/README.md)。
- **开始使用与收尾：** 内容按阅读顺序轻量显现，回到平静的下载入口；没有滚动劫持或自动播放。
- **可访问性：** 主叙事无需 hydration；增强失效仍可阅读，尊重 reduced motion，交互支持键盘和可见焦点。

桌面内容与插画共享居中的 1664px 上限。录屏按原始宽高比完整缩放，不以固定高度裁切。首屏按原画比例计算高度，宽屏两侧融合背景，避免 4K 下固定高度造成大幅裁切。960px 以下切换独立手机构图，图片放在正文下方；实际断点与样式以 [global.css](../../web/src/styles/global.css) 为准。

## 生产资产

生成工具为 `repochan image gen`，配置与服务回传模型为 `gpt-image-2.5-sunburst`。共同人物参考是 `ord-foundation-001` / `v2026-07-31T10-59-43-937Z`。这些是连续环境与角色复合插画，没有从整页设计稿截出产品 UI。

| 消费文件 | 尺寸 | 订单 / 不可变结果版本 |
| --- | --- | --- |
| [hero-desktop.webp](../../web/public/assets/refresh/hero-desktop.webp) | 2560 × 1600 | `ord-web-refresh-production-hero-desktop-20260910-001` / `v2026-09-10T09-33-23-205Z` |
| [hero-mobile.webp](../../web/public/assets/refresh/hero-mobile.webp) | 1024 × 1024 | `ord-web-refresh-production-hero-mobile-20260910-001` / `v2026-09-10T09-35-21-347Z` |
| [closing.webp](../../web/public/assets/refresh/closing.webp) | 1536 × 1024 | `ord-web-refresh-production-closing-20260910-001` / `v2026-09-10T09-37-14-801Z` |

旧产品模型另用四份桌面端素材：`rambledesk-app-icon.webp` 与 `rambelle-{idle,recording,archived}.webp`。它们保留供历史模型使用，当前主页不加载这些文件。

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

2026-09-10 的原始页面检查覆盖中英文、360–3840 CSS px 的 24 个响应式案例。此为历史验证记录；当前版本应重新验证中英文、响应式尺寸、键盘导航、播放与下载入口。视口测试不等于物理手机或不同显示器缩放下的人类审美验收。

文案与版本入口、开发和部署命令统一见 [web/README.md](../../web/README.md)。当前演示由 [WorkbenchDemos.svelte](../../web/src/components/WorkbenchDemos.svelte) 承担。更新产品能力时同时检查中英文、录制来源、发布状态、键盘操作和媒体加载行为，避免开发预览与稳定下载的功能口径混淆。

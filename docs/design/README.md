# 宣传页设计与素材

2026-09-10 升级以「把注意力，留给判断」为主题，沿用 README 的产品叙事：Agent 写清体验单，人直接体验、口述和编辑，再把不可变反馈包交回 Agent。网页让访客理解产品、看见反馈流程并找到下载入口。

## 视觉与交互意图

品牌继承 RepoChan 的 Rambelle：银白长发、冰蓝眼睛、银白制服与空心六边形。她在首屏递交透明档案板，结尾收起档案；动作对应请求与反馈的交付。浅色纸面、玻璃边缘和档案索引来自既有角色世界，信息层级遵循清楚、克制、诚实的内容设计原则。

全页按「明亮首屏 → 体验单 → 深色反馈示例 → 接入步骤 → 浅色收尾」展开。只用一个深色主段突出反馈过程。标题、状态、按钮、示例与结果均为真实 HTML，生产插画不承载产品信息。

- **导航与首屏：** 大字建立价值，右侧近景角色建立身份；导航离开首屏后增加底色。入场动效一次完成，不循环争夺注意力。
- **体验单：** 并列呈现「刚才发生了什么」与「请你体验或确认什么」，以具体设置页案例连接后续示例。
- **反馈示例：** 手动切换体验单、口述与截图、提交反馈三个步骤；只过渡当前面板，不模拟实时录音或 Agent 回复。明确标为交互示例，另附旧版真实 GIF。
- **开始使用与收尾：** 内容按阅读顺序轻量显现，回到平静的下载入口；没有滚动劫持或自动播放。
- **可访问性：** 主叙事无需 hydration；增强失效仍可阅读，尊重 reduced motion，交互支持键盘和可见焦点。

桌面内容与插画共享居中的 1664px 上限，反馈区使用 1080px 内层范围。首屏按原画比例计算高度，宽屏两侧融合背景，避免 4K 下固定高度造成大幅裁切。960px 以下切换独立手机构图，图片放在正文下方；实际断点与样式以 [global.css](../../web/src/styles/global.css) 为准。

## 生产资产

生成工具为 `repochan image gen`，配置与服务回传模型为 `gpt-image-2.5-sunburst`。共同人物参考是 `ord-foundation-001` / `v2026-07-31T10-59-43-937Z`。这些是连续环境与角色复合插画，没有从整页设计稿截出产品 UI。

| 消费文件 | 尺寸 | 订单 / 不可变结果版本 |
| --- | --- | --- |
| [hero-desktop.webp](../../web/public/assets/refresh/hero-desktop.webp) | 2560 × 1600 | `ord-web-refresh-production-hero-desktop-20260910-001` / `v2026-09-10T09-33-23-205Z` |
| [hero-mobile.webp](../../web/public/assets/refresh/hero-mobile.webp) | 1024 × 1024 | `ord-web-refresh-production-hero-mobile-20260910-001` / `v2026-09-10T09-35-21-347Z` |
| [closing.webp](../../web/public/assets/refresh/closing.webp) | 1536 × 1024 | `ord-web-refresh-production-closing-20260910-001` / `v2026-09-10T09-37-14-801Z` |

SHA-256：

```text
hero-desktop.webp 21461b47b73fba79d27a1a97621af1b03b5a5078ca9913c86508affeb1c1e3b3
hero-mobile.webp  aed8fa376d836259b38abb8638b3bd6e5ce6512ce307144098f73e5a325cf118
closing.webp      e36d1776b19045977c64cce8d7d7edcadedaf517476439871a40d3d0e45da4f7
```

[几何品牌标记](../../web/public/assets/refresh/rambledesk-mark.svg) 为手写 SVG，只用于网页导航、页脚与 favicon。README 的 Q 版收尾图另见[品牌图片来源](../social/README.md)。完整提示词与生成原图保存在 RepoChan 对应订单版本；设计迭代原稿和截图保留在设计者本机的 `.local-artifacts/marketing-refresh-20260910/`，不作为运行依赖。

## 实现验证与维护

设计方向已获用户确认。2026-09-10 实现检查覆盖中英文、360–3840 CSS px 的 24 个响应式案例，包括断点两侧、4K 居中、首屏图框与横向溢出；静态检查零错误/警告，构建输出两种语言。视口测试不等于物理手机或不同显示器缩放下的人类审美验收。

文案与版本入口、示例数据、开发和部署命令统一见 [web/README.md](../../web/README.md)。新产品录屏可替换旧 GIF，但应保留手动播放、文字说明和准确的反馈交付状态。

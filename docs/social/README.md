# 品牌图片来源

## readme-header-dark.webp / readme-header-light.webp

0.5.0 中英文 README 共用的头图，根据 GitHub 的明暗主题选择对应版本。来源为用户于 2026-10-08 提供的 `RambleDesk-README-0.5.0-draft.zip`，内容为 Rambelle 头像与 RambleDesk 字标。

| 消费文件 | 尺寸 | 大小 |
|---|---|---|
| [readme-header-dark.webp](readme-header-dark.webp) | 1600 × 360 | 45,104 bytes |
| [readme-header-light.webp](readme-header-light.webp) | 1600 × 360 | 44,758 bytes |

由包内同名 PNG 转为无损 WebP（method 6、exact），未缩放或改画。解码后的全部 RGBA 像素与原图一致，保留透明背景；两张合计比原 PNG 减少约 40%。原始压缩包及预览 HTML 不作为仓库发布资产。

## rambelle-chibi-footer.webp

中英文 README 共用的结尾横幅：Q 版 Rambelle 在小型反馈工作台递出透明档案包，以轻盈的品牌画面收束阅读。图中的两行文字为 `RambleDesk` 和 `Rambling is all you need.`，不承载产品界面或状态承诺。

| 字段 | 记录 |
|---|---|
| 消费文件 | [rambelle-chibi-footer.webp](rambelle-chibi-footer.webp) |
| 尺寸 / 大小 | 1600 × 640；76,706 bytes |
| SHA-256 | `79681c8473f1ed60b1a50a750d3afde17e8a2b579246a5ca8ded15007bad3455` |
| 生产订单 | `ord-readme-chibi-footer-20260910-001` |
| 不可变结果版本 | `v2026-09-10T11-44-55-503Z` |
| 人物参考 | `ord-foundation-001` / `v2026-07-31T10-59-43-937Z` |
| 模板 | `official/readme-banner-21x9`；按本次需求覆盖默认画幅为 5:2 |
| 工具 / 模型 | `repochan image gen` / `gpt-image-2.5-sunburst`（配置与服务回传值） |
| 端点 / 模式 | `task-api-1-cn-65535` / `openai-async` |
| 原始图片 | `generated-2026-09-10T11-43-11.png`，2560 × 1024；保存在上述订单结果版本中 |
| 装配 | `repochan image edit compress`，WebP quality 88，max-width 1600 |

2026-09-10 生成。完整提示词保存在订单结果的 `generationPrompt` 中。原 PNG 与消费 WebP 已目视检查：Q 版身份、手势、留白和两行文字清楚，压缩后没有明显主体质量损伤。中英文 README 已采用同一张图片。

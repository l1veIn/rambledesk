# 官网工作台演示媒体

2026-10-12，从 [README 演示](../../../../docs/screenshots/README.md) 的同一批真实应用截图生成。直接使用原始 JPEG 帧的无损 PNG 展开序列，避免再次量化 GIF；原始截图和 `timings.json` 位于本地忽略目录 `.local-artifacts/readme-demos-20261012/{en,zh-CN}/{workbench}/`。

每段 6 秒、H.264 / yuv420p、20 fps、无音轨、MP4 faststart。原步骤停留时间均为 50 ms 的倍数，因此只重复真实帧，不插值、不改字、不裁切。封面来自同组 `frame000.jpg`，Lanczos 缩放至 1280 宽，WebP quality 80 / method 6。视频保留 1600 宽；yuv420p 高度取偶数。页面仅在用户点击后加载播放，不自动播放或循环。

| 文件名（均有 `.mp4` 和 `.webp`） | 视频尺寸 | 英文视频 / 封面（字节） | 中文视频 / 封面（字节） |
| --- | --- | ---: | ---: |
| `document-review` | 1600 × 1334 | 227,520 / 28,758 | 215,110 / 28,650 |
| `visual-feedback` | 1600 × 1000 | 115,728 / 15,394 | 87,782 / 13,254 |
| `brainstorm` | 1600 × 1334 | 388,337 / 29,936 | 348,316 / 25,344 |
| `free-feedback` | 1600 × 1000 | 105,790 / 14,288 | 98,573 / 12,270 |

16 个媒体文件共 **1,755,050 字节**；封面分别为 1280 × 1067 或 1280 × 800。公开路径为 `/assets/workbenches/{en,zh-CN}/{name}.mp4`，封面使用同名 `.webp`。

编码命令（FFmpeg 7.0.2；`frames.ffconcat` 按 `timings.json` 列出真实帧、`option framerate 100` 和每帧 duration）：

```sh
ffmpeg -f concat -safe 0 -i frames.ffconcat -vf "scale=1600:-2:flags=lanczos,setsar=1,tpad=stop_mode=clone:stop_duration=6,fps=20" -t 6 -an -c:v libx264 -preset slow -crf 22 -pix_fmt yuv420p -movflags +faststart output.mp4
```

封面生成（Pillow 11.1.0）：

```python
from PIL import Image
with Image.open("frame000.jpg") as image:
    size = (1280, round(image.height * 1280 / image.width))
    image.convert("RGB").resize(size, Image.Resampling.LANCZOS).save(
        "output.webp", "WEBP", quality=80, method=6
    )
```

已解码全部视频帧并核对 120 帧 / 6 秒、无音轨、`moov` 位于 `mdat` 前；每段末帧另作视觉检查。源文件 SHA256 保持不变，输出哈希、命令及规格记录于本地 `.local-artifacts/readme-demos-20261012/web-media-audit.json`。

这些是带停留时间的真实操作截图，不是连续录像；原截图的柔化仍在。头脑风暴使用预置第二轮材料，展示编辑、分组、选择和组合意图，没有录制真实 Agent 生成下一轮；英文末帧聚焦空说明框，中文末帧含说明。文档审阅例稿中的夸大句是待改写的审阅材料，不是产品能力声明。

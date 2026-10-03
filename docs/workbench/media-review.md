# 音视频审阅工作台

`media_review` v1 是普通反馈工作台。Agent 提供一份不可变音频或视频及固定毫秒时长，人可以主动播放、定位时间点或区间、填写批注与整体意见。它不要求审批结论，也不提供转码、剪辑或自动播放。

交互参考 [Frame.io V4 的媒体批注说明](https://help.frame.io/en/articles/9105251-commenting-on-your-media)：播放器与评论侧栏并列，开始填写批注会暂停播放，点击评论回到对应时间，区间两端可以调整。当前实现支持时间点、区间、时间/最新/最早排序和点击定位；不包含 Frame.io 的协作、版本比较或范围循环播放功能。

## 请求与冻结材料

```json
{
  "workbench": {
    "type": "media_review",
    "version": 1,
    "data": {
      "title": "审阅片头节奏",
      "source_version": "intro-v3",
      "media_kind": "video",
      "media_file_name": "intro.webm",
      "duration_ms": 8000
    }
  },
  "attachments": [{ "file_name": "intro.webm", "path": "/absolute/path/intro.webm" }]
}
```

`title` 为 1–200 个 Unicode 标量，`source_version` 为 1–128 个；都必须有可见内容。`media_kind` 仅接受 `audio` 或 `video`。`duration_ms` 为 1–86,400,000 的整数，是不可变的时间坐标基准。

`media_file_name` 必须是合法的单一文件名，不含路径分隔符、NUL 或首尾 Unicode 空白，最多 255 UTF-8 字节。它必须精确、唯一地匹配一个已冻结请求附件，且媒体 MIME 类型与 `media_kind` 一致。请求建立时保存材料；播放器经现有受控 `readRequestAttachment` 能力读取，不使用请求后可能变化的路径，不接受任意播放 URL。

支持的容器和 MIME：

| 媒体 | 扩展名 | MIME |
| --- | --- | --- |
| 音频 | `.wav` | `audio/wav` |
| 音频 | `.mp3` | `audio/mpeg` |
| 音频 | `.m4a` | `audio/mp4` |
| 音频 | `.ogg`、`.opus` | `audio/ogg` |
| 音频 | `.weba` | `audio/webm` |
| 视频 | `.mp4` | `video/mp4` |
| 视频 | `.webm` | `video/webm` |
| 视频 | `.ogv` | `video/ogg` |

后端校验容器特征、文件名和 MIME；浏览器仍需支持材料实际使用的编码。无法播放会显示错误并保留草稿，应由 Agent 提供支持的材料建立新请求。实际时长与声明值相差超过 250 毫秒或 1%（取较大者）时显示提示并阻止新增时间批注，保留既有位置，不自动修正时间基准。仍可主动播放、回看已有意见或提交整体说明。

## 批注与提交

```json
{
  "type": "media_review",
  "comments": [
    { "id": "intro-point", "start_ms": 1200, "end_ms": null, "body": "这里的提示音偏响。" },
    { "id": "intro-range", "start_ms": 3000, "end_ms": 4500, "body": "这段节奏可以稍慢。" }
  ]
}
```

时间点的 `end_ms` 为 `null`，位置允许 `0 <= start_ms <= duration_ms`。区间使用闭合两端坐标，要求 `0 <= start_ms < end_ms <= duration_ms`。所有位置必须为整数毫秒；未知字段、越界位置、重复 ID 均拒绝。ID 遵循 `[a-z0-9][a-z0-9_-]{0,63}`，最多 500 条批注，正文最多 4000 个 Unicode 标量。

空正文允许保存草稿；提交时每条已有批注都须有可见内容，或先删除空批注。正文复用共享编辑器、语音目标和附件引用机制。整体说明沿用右侧共享反馈正文；没有时间批注时可以只提交整体说明。取消没有类型结果。

提交结果为 `{ "source_version": "intro-v3", "duration_ms": 8000, "comments": [...] }`。批注位置、正文和创建顺序会保存；侧栏排序是显示选择，不重排保存结果。时间点与区间标记可回到原材料位置。历史回看可主动播放和定位，编辑、添加、删除受宿主只读状态限制。

## 播放与资源生命周期

首版复用 20 MiB 请求附件预算。每个请求控制器按精确材料身份单次读取并缓存 bounded Blob URL；普通/全屏切换共享这一份缓存。播放器不自动播放，开始批注或提交会暂停。控制器保证只有一个活动播放器；离开视图会暂停并解除媒体源，离开请求会撤销 URL，迟到的读取结果不会再创建播放资源。

这里没有 HTTP Range 流式播放或新 URL 凭据。现有受控接口需应用认证，浏览器媒体标签不能直接设置 Bearer 头；Blob 仅承接已经授权且有大小限制的读取。大于 20 MiB 的材料需先由 Agent 缩小或分段并创建新请求，工作台本身不转码。

开发预览选择 `workspace=media_review-audio` 或 `workspace=media_review-video`，包含真实一秒 WAV/VP8 WebM。人工演练使用 `playground/workbenches/fixtures/11-media-audio.json` 和 `12-media-video.json`，包含八秒 WAV、VP8/Opus WebM 与独立演练说明。测试素材和自动化结果不替代设备上的实际解码、播放和人工验收。

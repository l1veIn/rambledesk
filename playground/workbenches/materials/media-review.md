# 音视频审阅演练

两份材料都是真实的八秒测试信号：单声道 WAV 提示音，以及 VP8/Opus WebM 运动测试图案。它们用于验收实际播放器、声音与时间定位，不代表真实业务内容或已完成的人工验收。

1. 打开请求后确认没有自动播放。主动播放，测试暂停、音量和时间轴定位。
2. 在当前时间点添加意见。时间轴获得焦点时用 I、O 标记区间两端，也可拖动区间滑杆；添加区间批注应暂停播放。
3. 输入意见，使用共享语音和附件工具。清空正文可保存草稿，提交前必须填写或删除空批注。
4. 按时间、最新、最早排序批注，点击卡片或时间标记定位；排序不改变保存顺序或时间位置。
5. 切换普通与全屏评审，确认同一份草稿和材料继续使用。离开请求时不得继续后台播放。
6. 提交后从历史回看，主动播放和点击批注仍可用，创建、编辑和删除应不可用。仅提交整体反馈也合法。

声明时长固定为 8000 毫秒。浏览器无法解码，或实际时长明显不符时应清晰提示并保留草稿；不要修改已有时间位置来掩盖材料问题。

可用 FFmpeg 生成相同技术材料（无远程下载）：

```sh
ffmpeg -f lavfi -i 'sine=frequency=440:sample_rate=8000:duration=8' -ac 1 -c:a pcm_s16le review-tone.wav
ffmpeg -f lavfi -i 'testsrc2=size=320x180:rate=10:duration=8' -f lavfi -i 'sine=frequency=660:sample_rate=48000:duration=8' -c:v libvpx -b:v 120k -deadline realtime -cpu-used 8 -threads 1 -c:a libopus -b:a 24k -shortest review-motion.webm
```

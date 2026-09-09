# 真实 Cooking 验收

2026-09-09，工作区基于 rc3 `b7ab9d088aa2a67322e8ee9b1c697efe9075f1ce`，使用本轮 Cooking / Publisher / Draft controller。

## 结果

**通过：真实整理 → 独立变体 → 发布 → HTTP/SQLite/包哈希核对。**

使用用户已有配置 `deepseek / deepseek-v4-flash`、`https://api.deepseek.com/v1`、reasoning `minimal` 和已有自定义 system prompt。只读载入配置；API key 仅在验收进程中使用，不输出或落盘，没有修改用户启用状态（原为关闭）。全部待整理内容为本轮生成的测试反馈。

[实际输入、输出与时间](evidence/cooking-live.json) 记录一次完整通过的结果：6.671 秒完成准备、保存和真实模型整理。原文关于“两秒延迟”“网络原因不确定”“保存中提示”“失败保留输入并重试”的含义得到保留。自定义 prompt 的输出包含请求背景；本次保留该配置行为，没有据此修改用户 prompt。

验收直接使用真实 `createCookingController`、`createDraftController`、`createPublisherController` 和状态 owners。Node 测试宿主的 transport adapter 将每个命令发送到隔离 HTTP 服务；模型调用使用生产 `cooking.ts` 的实际 fetch 和 SDK，不替换模型返回。

断言并核对：

- 原始 `document_json` 与 Markdown 草稿保持原样，保存 revision 为 2。
- Cooking preview 的 request id、saved revision、original 与已保存源稿对应。
- Publisher 使用已有 preview 发布，不额外调用一次模型。
- 最终包的原稿、变体、模型与源 revision 对应，HTTP 投影与 SQLite 和磁盘包一致。
- [磁盘核对结果](evidence/cooking-package2.json) 的正文与 manifest SHA-256 通过。两次临时服务均正常停止，临时令牌已删除。

首次探针已完成真实模型调用和发布，但测试错误地要求包 Markdown 与 preview 的结尾换行完全相同，导致断言失败。`package.rs` 的既有合同是为正文与原稿补结尾换行；按该合同修正断言后，在新隔离请求上完整复验通过。没有为通过测试修改生产格式行为。[首次包的独立核对](evidence/cooking-package.json) 同样通过。

## 重复运行与限制

[显式 live 验收入口](../../apps/desktop/src/dev/cookingAcceptance.test.ts) 只在三个 `RAMBLEDESK_COOKING_ACCEPTANCE_*` 环境变量都存在时运行；普通 Vitest 跳过，不访问用户配置或模型。CONFIG 指向已有 WebKit localStorage SQLite，FIXTURE 指向新建隔离夹具 manifest，OUTPUT 指向不含凭据的结果 JSON。该配置读取器限定本次 macOS WebKit 存储格式，并非新增产品配置机制。

这是模型、controller 与真实后端组合证据。没有运行浏览器 CORS 或 Tauri HTTP 插件分支，不等于所有提供商兼容、原生可视 Cooking 点击流程或完整 M6 已通过。语音设备、Agent 多轮闭环、安装升级与真机待验分别保留在全局计划中。

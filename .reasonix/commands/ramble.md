# /ramble — 通过 RambleDesk 发起一轮人类反馈

默认走 RambleDesk 反馈循环：把需要人类真实判断、体验、审阅的请求通过
`request_feedback` 持久化发出，等待人类在桌面工作台中 ramble 并提交反馈包，
再用 `get_feedback` 读取不可变反馈并实现。不要把这些请求散落在聊天里。

## 执行流程

1. **发请求**：调用 `mcp-tool:rambledesk/request_feedback`，参数如下：
   - `host_id`: `reasonix`
   - `host_session_id`: 当前会话 id（没有则用可读标识，如 `reasonix-<日期>`）
   - `title`: 简短标题，让人类在 Inbox 里一眼看懂
   - `what_happened`: 说明背景——我（宿主 agent）正在做什么、为什么需要这次反馈、
     人类应该带着什么视角去使用/检查
   - `actions`: 明确、可执行的动作清单（人类要做的每件事一条）
   - `attachments`: 需要人类审阅的产物（markdown 用 `.md`，图片用 base64）
   - `context_refs`: 相关的仓库文档/文件引用
   - `allow_finish`: **默认不传**（保持 false）。只有请求只是"请批准/拒绝某个简单
     决定、不需要反馈正文"时才设 `true`，此时必须同时给 `final_summary`
   - `request_id`: 不传，让服务器生成 UUID；或传合法的 UUID
2. **等待**：创建成功后**不要轮询**。若有交互确认工具（ask / ask_choice），用它
   等待人类完成；否则结束回合，等通知后再继续。
3. **读取**：人类完成/确认后，调用 `mcp-tool:rambledesk/get_feedback` 传入
   `request_id`，然后按返回路径读取 `feedback.md`（完整反馈）与附件。
4. **实现**：按反馈逐条实现；需要再次确认或审阅时，从第 1 步重复。
5. **取消**：人类明确要求放弃时，用 `cancel_feedback` 取消等待中的请求。

## 原则

- 反馈请求必须先落盘：请求创建成功即持久化，连接断开/重启不丢失。
- 需要详细反馈（体验、检查、意见）的请求绝不设 `allow_finish`，人类应提交反馈
  正文而非捷径式批准。
- 一次请求对应一个明确主题；把多主题拆成多次请求，避免反馈包臃肿。
- 读取反馈后先复述理解再动手，若反馈有歧义，用下一轮 ramble 澄清而不是猜。

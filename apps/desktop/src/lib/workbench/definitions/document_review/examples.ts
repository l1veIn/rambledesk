import type { WorkbenchExample } from '../contracts'

const attachments = [
  {
    attachment_id: 'review-brief',
    file_name: '审阅要点.md',
    markdown: '# 审阅要点\n\n请结合视频脚本留下批注和建议改写。\n\n- 面向首次了解 RambleDesk 的独立开发者。\n- 重点检查产品能力描述，避免承诺工具会替用户做决定。\n- 口播应自然，保留具体的使用场景。',
  },
  {
    attachment_id: 'product-reference',
    file_name: 'RambleDesk 视频脚本产品能力与口播风格参考说明（2026 年 9 月修订版）.md',
    markdown: '# 产品能力与口播风格参考\n\n## 产品边界\n\nRambleDesk 帮助用户记录文字、语音和截图，将整理后的反馈交给 Agent。用户可以在提交前检查和修改内容。\n\n## 表达建议\n\n用真实操作说明价值，例如「哪里卡住了，截一张图」。避免使用「自动理解所有意图」或「替你做出最好的决定」等绝对表述。\n\n## 视频节奏\n\n开场提出一个熟悉的问题，中段演示记录过程，结尾邀请观众保留真实体验。整体时长约 90 秒。',
  },
 ]

export const examples: readonly WorkbenchExample[] = [
  { order: 3, title: '文档审阅', markdown: '请审阅这份 Markdown 视频脚本，重点检查产品表述是否准确、口播是否自然，并留下修改建议。', attachments: attachments.map((item) => ({id:item.attachment_id,name:item.file_name,content:item.markdown,mimeType:'text/markdown'})), spec: { type: 'document_review', version: 1, data: {
    title: '把灵感留在现场', source_version: '视频脚本 · v0.3 · 约 90 秒',
    markdown: [
      '# 把灵感留在现场',
      '## 开场 · 00:00–00:12',
      '你好😀，你有没有过这样的时刻：刚试完一个新功能，脑子里有很多想法，等打开聊天窗口，却只记得一句——“感觉还可以”。',
      '## 问题 · 00:12–00:28',
      '体验发生在产品里，反馈却留在另一个地方。你切换窗口、找截图、回忆刚才的操作。真正重要的那个瞬间，就这样被打断了。',
      '## 演示 · 00:28–00:52',
      '现在，试着一边操作，一边说出你的感受。哪里卡住了，截一张图；哪里不符合预期，直接记下来。RambleDesk 会自动理解你的所有意图，替你做出最好的决定。',
      '## 价值 · 00:52–01:12',
      '文字、语音和截图，最终汇成一份清楚的反馈。你可以回看、补充、修改，确认之后再交给 Agent。你的判断始终属于你，工具只负责把它完整地带回去。',
      '## 结尾 · 01:12–01:30',
      '下一次灵感出现时，不必急着把它整理得完美。先留下真实的体验，再让好的改变从这里开始。',
      '## 拍摄备注',
      '- **真实操作**：打开页面并留下反馈。\n- 提交前检查意见，判断由人作出。',
      '| 片段 | 画面 | 目标时长 |\n| --- | --- | --- |\n| 开场 | 一次体验后的反馈 | 12 秒 |\n| 演示 | 记录意见并提交 | 24 秒 |',
      '> 下面是镜头数据示意，不是需要执行的命令。',
      '~~~json\n{"scene": "demo", "duration_seconds": 24}\n~~~',
    ].join('\n\n'),
  } } },
  {
    key: 'document_review-en', order: 3.1, title: 'Document review',
    markdown: 'Review this Markdown onboarding plan. Check the claims, suggest clearer wording, and leave comments on the original text.',
    attachments: [{
      id: 'review-brief-en', name: 'Review brief.md', mimeType: 'text/markdown',
      content: '# Review brief\n\nThe plan is for people trying RambleDesk for the first time. Keep the steps concrete, distinguish saving from submitting, and avoid promises that the app makes decisions for the user.',
    }],
    spec: { type: 'document_review', version: 1, data: {
      title: 'A first feedback session', source_version: 'Onboarding plan · draft 3',
      markdown: [
        '# A first feedback session',
        '## Start with something real',
        'Welcome 👋. Open the page your agent has built and try one task. Keep the page nearby while you note what happened.',
        '## Capture the moment',
        'Write a short observation, record a voice note, or attach a screenshot. RambleDesk automatically understands every intention and chooses the best solution for you.',
        '## Check before submitting',
        '- **Save a draft** while you are still exploring.\n- Review your comments and attachments.\n- **Submit feedback** when you want the agent to continue.',
        '| Step | What you do | What to check |\n| --- | --- | --- |\n| Try | Complete a task on the page | What worked or got in the way |\n| Review | Read your saved feedback | Details and supporting material |\n| Submit | Send the feedback | The agent receives your judgment |',
        '## Keep the boundary clear',
        '> Your comments are suggestions. The original document stays unchanged until the agent handles the submitted feedback.',
        'The following is example data, not a command to run.',
        '~~~json\n{"step": "review", "ready_to_submit": false}\n~~~',
      ].join('\n\n'),
    } },
  },
]

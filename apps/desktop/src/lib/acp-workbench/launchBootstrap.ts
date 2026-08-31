const launchKickoffContract = 'No task brief has been provided for this new RambleDesk Session. Before any substantive work, call request_feedback exactly once to ask the human in RambleDesk for their goal, relevant context and materials, constraints, desired output, priorities, and completion criteria. End this turn immediately after request_feedback; RambleDesk will keep the Session open and resume it when the human responds. Do not ask for the task brief in plain chat, guess the task, or start work.'

export const launchBootstrapMarkdown = `# New Ramble

${launchKickoffContract}`

export const launchBootstrapDocumentJson = JSON.stringify({
  type: 'doc',
  content: [
    { type: 'heading', attrs: { level: 1 }, content: [{ type: 'text', text: 'New Ramble' }] },
    {
      type: 'paragraph',
      content: [{ type: 'text', text: launchKickoffContract }],
    },
  ],
})

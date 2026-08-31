export const launchBootstrapMarkdown = `# New Ramble

Call \`request_feedback\` now to ask the human what they want to work on. Do not guess their intent or start work before their feedback is submitted.`

export const launchBootstrapDocumentJson = JSON.stringify({
  type: 'doc',
  content: [
    { type: 'heading', attrs: { level: 1 }, content: [{ type: 'text', text: 'New Ramble' }] },
    {
      type: 'paragraph',
      content: [{
        type: 'text',
        text: 'Call request_feedback now to ask the human what they want to work on. Do not guess their intent or start work before their feedback is submitted.',
      }],
    },
  ],
})

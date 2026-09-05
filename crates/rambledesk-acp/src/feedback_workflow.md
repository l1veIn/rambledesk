<rambledesk_session_context>
You are running inside a RambleDesk-managed ACP session. The following is the built-in communication workflow for this session.

Work on the user's task normally. When you need a decision, clarification, hands-on check, or review of completed work, create a RambleDesk feedback request using your own command/terminal tool. Unless the user explicitly asks otherwise, use this workflow when handing work back to the user. You do not need to be reminded to use RambleDesk.

The application executable is available in the RAMBLEDESK_COMMAND environment variable. Invoke it with shell-appropriate quoting:
- Bash/sh: "$RAMBLEDESK_COMMAND" feedback request --input /absolute/path/request.json
- PowerShell: & $env:RAMBLEDESK_COMMAND feedback request --input C:\absolute\path\request.json
The same command accepts `feedback get --request-id <id>` and `feedback recover --request-id <id>`. `feedback recover` without an ID recovers this session's existing request. `feedback --help` documents the input. You may pipe UTF-8 JSON with `--input -`; a UTF-8 file avoids shell escaping problems.

Request JSON example:
{"title":"Review the result","what_happened":"Describe the concrete work and what feedback is needed.","actions":[{"id":"review","instruction":"Check the result and tell me what to change."}]}
Optional fields: request_id (UUID, reuse for retries), context_refs [{label,uri}], attachments [{file_name,path}] for existing absolute local images or Markdown, allow_finish, final_summary. Set allow_finish:true and final_summary when the user can confirm the overall task is complete. Use the user's language and provide enough context to review the result.

The command prints one JSON result and exits. Retain its request_id. Once the request is saved, end your current Agent turn immediately. Do not poll, sleep, wait for a host confirmation, or call any tool that blocks on human input. The user responds in the Ramble page; RambleDesk automatically continues this same Agent context. On continuation, run `feedback get` with the original ID and read the returned feedback_package, including feedback and attachment references. If the request was cancelled, respect that result. If the user finishes the task, do not create another confirmation request.

If delivery is uncertain, preserve the returned request_id and recover it before any retry. Never create a replacement request merely because a command failed or the session reconnected. Missing/revoked capability means reconnect this session; do not fall back to an external endpoint.

For this managed session, these command instructions take precedence over the transport-selection section of any installed Ramble skill or adapter. Do not use external Ramble MCP tools, native Pi/dsh Ramble tools, or a separately chosen host/session identifier: they can route feedback outside this conversation. The command inherits its private capability automatically. Do not read, print, store, or override its token/URL environment variables. Existing unrelated MCP tools and skills remain available for the user's work.
</rambledesk_session_context>

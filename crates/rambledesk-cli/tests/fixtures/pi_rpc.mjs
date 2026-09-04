// Real Node subprocess exercising the production managed extension through the
// production RPC wrapper. No provider/model calls or global configuration.
import { pathToFileURL } from "node:url";
import { createInterface } from "node:readline";
import { spawn, spawnSync } from "node:child_process";
const args = process.argv.slice(2);
const extension = args[args.indexOf("--extension") + 1];
const tools = new Map(), handlers = new Map(), entries = [];
for (const name of ["RAMBLEDESK_MANAGED_PI_WRAPPER", "RAMBLEDESK_MANAGED_PI_COMMAND", "RAMBLEDESK_MANAGED_PI_ARGS", "RAMBLEDESK_MANAGED_PI_EXTENSION", "PI_ACP_PI_COMMAND"]) {
  if (process.env[name] !== undefined) throw new Error("Wrapper control environment leaked to Pi");
}
if (!args.includes("--no-themes") || !args.includes("fixture-session.json")) throw new Error("Original Pi arguments were lost");
const pi = {
  registerTool(tool) { tools.set(tool.name, tool); },
  on(name, handler) { handlers.set(name, handler); },
  appendEntry(customType, data) { entries.push({ type: "custom", customType, data }); },
  sendUserMessage() { throw new Error("The extension must never dispatch continuation"); },
};
const syntheticSecret = process.env.RAMBLEDESK_MANAGED_MCP_TOKEN;
await (await import(pathToFileURL(extension).href)).default(pi);
const inherited = spawnSync(process.execPath, ["-e", "process.stdout.write(JSON.stringify({url:process.env.RAMBLEDESK_MANAGED_MCP_URL!==undefined,token:process.env.RAMBLEDESK_MANAGED_MCP_TOKEN!==undefined}))"], { encoding: "utf8", windowsHide: true });
if (inherited.status !== 0 || inherited.stdout !== '{"url":false,"token":false}') throw new Error("A Pi child inherited managed credentials");
if (process.env.FIXTURE_HEARTBEAT) spawn(process.execPath, ["-e", "const fs=require('node:fs');setInterval(()=>fs.writeFileSync(process.env.FIXTURE_HEARTBEAT,String(Date.now())),10)"], { stdio: "inherit", windowsHide: true });
// Diagnostics deliberately contain the synthetic capability; the wrapper must
// drain them without exposing stderr to ACP or its parent process.
process.stderr.write(syntheticSecret);
const lines = createInterface({ input: process.stdin });
for await (const line of lines) {
  const input = JSON.parse(line);
  if (!Object.hasOwn(input, "id")) continue;
  let result;
  if (input.method === "initialize") result = { instructions: handlers.get("before_agent_start")({ systemPrompt: "fixture" }).systemPrompt };
  else if (input.method === "tools/list") result = { tools: [...tools.values()].map((tool) => ({ name: tool.name, inputSchema: tool.parameters })) };
  else if (input.method === "tools/call") {
    const tool = tools.get(input.params.name);
    try {
      const value = await tool.execute(`call-${input.id}`, input.params.arguments, undefined, undefined, { sessionManager: { getEntries: () => entries } });
      result = { content: value.content, structuredContent: value.details, isError: value.isError };
    } catch { result = { isError: true, content: [{ type: "text", text: "Managed binding unavailable; preserve request_id" }] }; }
  }
  process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id: input.id, result }) + "\n");
}
process.exit(0);

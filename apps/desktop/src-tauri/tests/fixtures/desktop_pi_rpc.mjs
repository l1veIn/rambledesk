// No model or global Pi installation: exercise Desktop's real wrapper, stdio
// handles, argument forwarding, and installed production managed extension.
import assert from "node:assert/strict";
import { createInterface } from "node:readline";
import { pathToFileURL } from "node:url";

const args = process.argv.slice(2);
assert.deepEqual(args.slice(0, 3), ["--mode", "rpc", "--no-themes"]);
for (const name of ["RAMBLEDESK_MANAGED_PI_WRAPPER", "RAMBLEDESK_MANAGED_PI_COMMAND", "RAMBLEDESK_MANAGED_PI_ARGS", "RAMBLEDESK_MANAGED_PI_EXTENSION", "PI_ACP_PI_COMMAND"]) {
  assert.equal(process.env[name], undefined, "wrapper launch controls are private");
}
const extension = args[args.indexOf("--extension") + 1];
const tools = new Map();
await (await import(pathToFileURL(extension).href)).default({
  registerTool(tool) { tools.set(tool.name, tool); },
  on() {},
  appendEntry() {},
});
for await (const line of createInterface({ input: process.stdin })) {
  const input = JSON.parse(line);
  assert.equal(input.type, "get_state");
  process.stdout.write(JSON.stringify({ id: input.id, type: "response", command: input.type, success: true, data: { tools: [...tools.keys()].sort() } }) + "\n");
}

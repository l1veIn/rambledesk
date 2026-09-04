import assert from "node:assert/strict";
import http from "node:http";
import { test } from "node:test";
import { spawn } from "node:child_process";
import { managedCapability, ManagedFeedbackClient } from "../managed-client.mjs";

test("managed capability cannot fall back to generic credentials or nonlocal URLs", () => {
  const token = "a".repeat(64);
  for (const url of ["https://127.0.0.1/mcp-managed", "http://localhost/mcp-managed", "http://192.168.1.2/mcp-managed", "http://127.0.0.1/mcp", "http://127.0.0.1/mcp-managed?token=secret", "http://secret@127.0.0.1/mcp-managed"]) {
    assert.throws(() => managedCapability({ RAMBLEDESK_MANAGED_MCP_URL: url, RAMBLEDESK_MANAGED_MCP_TOKEN: token }), (error) => !error.message.includes(token) && !error.message.includes("secret"));
  }
  assert.throws(() => new ManagedFeedbackClient({ RAMBLEDESK_LOCAL_SERVER_TOKEN: token }));
});

test("managed loading consumes child credentials and disables generic or duplicate tools in either order", async () => {
  const token = "c".repeat(64);
  let calls = 0;
  const server = http.createServer(async (request, response) => {
    if (request.headers.authorization !== `Bearer ${token}`) { response.writeHead(401); response.end(); return; }
    let text = "";
    for await (const chunk of request) text += chunk;
    const message = JSON.parse(text);
    if (!Object.hasOwn(message, "id")) { response.writeHead(202); response.end(); return; }
    let result;
    if (message.method === "initialize") result = { instructions: "End this turn after request_feedback." };
    else if (message.method === "tools/list") result = { tools: ["request_feedback", "get_feedback", "recover_feedback"].map((name) => ({ name, description: name, inputSchema: { type: "object", properties: {} } })) };
    else { calls++; result = { content: [{ type: "text", text: "waiting" }], structuredContent: { request_id: message.params.arguments.request_id, status: "waiting" } }; }
    response.writeHead(200, { "Content-Type": "application/json", "Mcp-Session-Id": "private-fixture" });
    response.end(JSON.stringify({ jsonrpc: "2.0", id: message.id, result }));
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    for (const order of ["generic-first", "managed-first"]) {
      const source = `
        import assert from 'node:assert/strict';
        import {spawnSync} from 'node:child_process';
        import generic from ${JSON.stringify(new URL("../index.js", import.meta.url).href)};
        import managed from ${JSON.stringify(new URL("../managed.mjs", import.meta.url).href)};
        const tools=[]; const pi={registerTool(tool){tools.push(tool)},on(){},appendEntry(){}};
        if (${JSON.stringify(order)}==='generic-first') generic(pi);
        const pending=managed(pi);
        assert.equal(process.env.RAMBLEDESK_MANAGED_MCP_URL,undefined);
        assert.equal(process.env.RAMBLEDESK_MANAGED_MCP_TOKEN,undefined);
        generic(pi);
        await pending;
        generic(pi);
        await (await import(${JSON.stringify(new URL("../managed.mjs?another-copy", import.meta.url).href)})).default(pi);
        assert.deepEqual(tools.map(tool=>tool.name),['request_feedback','get_feedback','recover_feedback']);
        const child=spawnSync(process.execPath,['-e',"process.stdout.write(JSON.stringify({url:process.env.RAMBLEDESK_MANAGED_MCP_URL!==undefined,token:process.env.RAMBLEDESK_MANAGED_MCP_TOKEN!==undefined}))"],{encoding:'utf8',windowsHide:true});
        assert.equal(child.status,0); assert.equal(child.stdout,'{"url":false,"token":false}');
        const value=await tools[0].execute('fixture-call',{},undefined,undefined,{});
        assert.equal(value.details.status,'waiting');
      `;
      await new Promise((resolve, reject) => {
        const child = spawn(process.execPath, ["--input-type=module", "-e", source], { env: { ...process.env, RAMBLEDESK_MANAGED_MCP_URL: `http://127.0.0.1:${server.address().port}/mcp-managed`, RAMBLEDESK_MANAGED_MCP_TOKEN: token }, windowsHide: true, stdio: ["ignore", "ignore", "pipe"] });
        let errors = "";
        child.stderr.on("data", (chunk) => { errors += chunk; });
        child.once("error", reject);
        child.once("close", (code) => code === 0 ? resolve() : reject(new Error(errors)));
      });
    }
    assert.equal(calls, 2);
  } finally { await new Promise((resolve) => server.close(resolve)); }
});

test("managed client refuses redirect responses instead of forwarding the capability", async () => {
  let redirected = false;
  const target = http.createServer((_request, response) => { redirected = true; response.end("{}"); });
  await new Promise((resolve) => target.listen(0, "127.0.0.1", resolve));
  const source = http.createServer((_request, response) => { response.writeHead(302, { Location: `http://127.0.0.1:${target.address().port}/mcp-managed` }); response.end(); });
  await new Promise((resolve) => source.listen(0, "127.0.0.1", resolve));
  try {
    const client = new ManagedFeedbackClient({ RAMBLEDESK_MANAGED_MCP_URL: `http://127.0.0.1:${source.address().port}/mcp-managed`, RAMBLEDESK_MANAGED_MCP_TOKEN: "a".repeat(64) });
    await assert.rejects(client.initialize());
    assert.equal(redirected, false);
  } finally {
    await Promise.all([new Promise((resolve) => source.close(resolve)), new Promise((resolve) => target.close(resolve))]);
  }
});

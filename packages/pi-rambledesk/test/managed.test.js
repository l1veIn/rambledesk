import assert from "node:assert/strict";
import http from "node:http";
import { test } from "node:test";
import { managedCapability, ManagedFeedbackClient } from "../managed-client.mjs";

test("managed capability cannot fall back to generic credentials or nonlocal URLs", () => {
  const token = "a".repeat(64);
  for (const url of ["https://127.0.0.1/mcp-managed", "http://localhost/mcp-managed", "http://192.168.1.2/mcp-managed", "http://127.0.0.1/mcp", "http://127.0.0.1/mcp-managed?token=secret", "http://secret@127.0.0.1/mcp-managed"]) {
    assert.throws(() => managedCapability({ RAMBLEDESK_MANAGED_MCP_URL: url, RAMBLEDESK_MANAGED_MCP_TOKEN: token }), (error) => !error.message.includes(token) && !error.message.includes("secret"));
  }
  assert.throws(() => new ManagedFeedbackClient({ RAMBLEDESK_LOCAL_SERVER_TOKEN: token }));
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

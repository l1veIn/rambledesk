import assert from "node:assert/strict";
import http from "node:http";
import { test } from "node:test";

import {
  feedbackToolResult,
  normalizeRequestParams,
  postFeedback,
  registerRambleDeskPiTools,
  resolveApiBaseUrl,
} from "../index.js";

test("normalizes Pi request params without model-supplied host identity", () => {
  const input = {
    host_session_id: "session-1",
    title: "Review",
    what_happened: "A workflow changed.",
    actions: [{ id: "check", instruction: "Check the workflow." }],
  };

  assert.deepEqual(normalizeRequestParams(input, { cwd: "/tmp/rambledesk" }), {
    request_id: undefined,
    host_id: "pi",
    host_session_id: "session-1",
    title: "Review",
    what_happened: "A workflow changed.",
    actions: [{ id: "check", instruction: "Check the workflow." }],
    context_refs: [],
    source_hint: "/tmp/rambledesk",
  });
});

test("resolves local API endpoint from explicit API or local server port", () => {
  assert.equal(
    resolveApiBaseUrl({ RAMBLEDESK_LOCAL_API_URL: "http://127.0.0.1:1/api/" }),
    "http://127.0.0.1:1/api",
  );
  assert.equal(resolveApiBaseUrl({ RAMBLEDESK_LOCAL_SERVER_PORT: "3" }), "http://127.0.0.1:3/api");
});

test("posts feedback requests with bearer token and Pi host header", async () => {
  const received = [];
  const server = http.createServer((request, response) => {
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => {
      body += chunk;
    });
    request.on("end", () => {
      received.push({
        url: request.url,
        authorization: request.headers.authorization,
        host: request.headers["x-rambledesk-host"],
        body: JSON.parse(body),
      });
      response.setHeader("content-type", "application/json");
      response.end(JSON.stringify({ request_id: "019", status: "waiting" }));
    });
  });

  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    const { port } = server.address();
    const result = await postFeedback(
      "request",
      { title: "Review" },
      undefined,
      {
        RAMBLEDESK_LOCAL_API_URL: `http://127.0.0.1:${port}/api`,
        RAMBLEDESK_LOCAL_SERVER_TOKEN: "test-token",
      },
    );

    assert.equal(result.request_id, "019");
    assert.equal(received[0].url, "/api/feedback/request");
    assert.equal(received[0].authorization, "Bearer test-token");
    assert.equal(received[0].host, "pi");
    assert.deepEqual(received[0].body, { title: "Review" });
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
});

test("retries a transient connection failure with the same request id", async () => {
  let attempts = 0;
  const receivedIds = [];
  const server = http.createServer((request, response) => {
    attempts += 1;
    if (attempts === 1) {
      // Simulate the observed failure mode: the server created the request but
      // the connection dropped before the response reached the client.
      request.on("data", () => {});
      request.destroy();
      return;
    }
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => {
      body += chunk;
    });
    request.on("end", () => {
      const parsed = JSON.parse(body);
      receivedIds.push(parsed.request_id);
      response.setHeader("content-type", "application/json");
      response.end(JSON.stringify({ request_id: parsed.request_id, status: "waiting" }));
    });
  });

  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    const { port } = server.address();
    const result = await postFeedback(
      "request",
      { request_id: "stable-id", title: "Review" },
      undefined,
      {
        RAMBLEDESK_LOCAL_API_URL: `http://127.0.0.1:${port}/api`,
        RAMBLEDESK_LOCAL_SERVER_TOKEN: "test-token",
      },
    );

    assert.equal(attempts, 2);
    assert.deepEqual(receivedIds, ["stable-id"]);
    assert.equal(result.request_id, "stable-id");
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
});

test("formats terminal feedback result text for Pi", () => {
  const result = feedbackToolResult({
    request_id: "019",
    status: "completed",
    feedback_package: { markdown: "done", attachment_paths: [] },
  });

  assert.match(result.content[0].text, /completed/);
  assert.equal(result.details.feedback_package.markdown, "done");
});

test("registers Pi tools and request tool waits for terminal package", async () => {
  const tools = [];
  registerRambleDeskPiTools({
    registerTool(tool) {
      tools.push(tool);
    },
  });

  assert.deepEqual(tools.map((tool) => tool.name), [
    "request_ramble_feedback",
    "get_ramble_feedback",
  ]);

  const calls = [];
  const server = http.createServer((request, response) => {
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => {
      body += chunk;
    });
    request.on("end", () => {
      calls.push({ url: request.url, body: body ? JSON.parse(body) : {} });
      response.setHeader("content-type", "application/json");
      if (request.url === "/api/feedback/request") {
        response.end(JSON.stringify({ request_id: "019", status: "waiting" }));
        return;
      }
      response.end(JSON.stringify({
        request_id: "019",
        status: "completed",
        feedback_package: {
          markdown: "human feedback",
          attachment_paths: ["/tmp/screenshot.png"],
        },
      }));
    });
  });

  const previousApiUrl = process.env.RAMBLEDESK_LOCAL_API_URL;
  const previousToken = process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN;
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    const { port } = server.address();
    process.env.RAMBLEDESK_LOCAL_API_URL = `http://127.0.0.1:${port}/api`;
    process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN = "test-token";
    const updates = [];
    const requestTool = tools.find((tool) => tool.name === "request_ramble_feedback");
    const result = await requestTool.execute(
      "call-1",
      {
        title: "Review",
        what_happened: "A workflow changed.",
        actions: [{ id: "check", instruction: "Check the workflow." }],
      },
      undefined,
      (update) => updates.push(update),
      { cwd: "/tmp/pi-worktree", sessionId: "pi-session" },
    );

    assert.equal(calls.length, 2);
    assert.equal(calls[0].url, "/api/feedback/request");
    assert.equal(calls[0].body.host_id, "pi");
    assert.equal(calls[0].body.host_session_id, "pi-session");
    assert.equal(calls[1].url, "/api/feedback/wait");
    assert.deepEqual(calls[1].body, { request_id: "019" });
    assert.match(updates[0].content[0].text, /waiting/);
    assert.match(result.content[0].text, /completed/);
    assert.equal(result.details.feedback_package.markdown, "human feedback");
  } finally {
    if (previousApiUrl === undefined) {
      delete process.env.RAMBLEDESK_LOCAL_API_URL;
    } else {
      process.env.RAMBLEDESK_LOCAL_API_URL = previousApiUrl;
    }
    if (previousToken === undefined) {
      delete process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN;
    } else {
      process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN = previousToken;
    }
    await new Promise((resolve) => server.close(resolve));
  }
});

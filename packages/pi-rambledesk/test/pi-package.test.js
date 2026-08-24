import assert from "node:assert/strict";
import http from "node:http";
import { test } from "node:test";

import {
  buildRambleKickoffMessage,
  checkHealth,
  feedbackToolResult,
  normalizeRequestParams,
  postFeedback,
  registerRambleDeskPiTools,
  resolveApiBaseUrl,
  restorePendingRequestId,
} from "../index.js";

test("normalizes Pi request params without model-supplied host identity", () => {
  const input = {
    host_session_id: "session-1",
    title: "Review",
    what_happened: "A workflow changed.",
    actions: [{ id: "check", instruction: "Check the workflow." }],
  };

  assert.deepEqual(normalizeRequestParams(input, { cwd: "/tmp/rambledesk" }, {}), {
    request_id: undefined,
    host_id: "pi",
    host_session_id: "session-1",
    title: "Review",
    what_happened: "A workflow changed.",
    actions: [{ id: "check", instruction: "Check the workflow." }],
    context_refs: [],
    attachments: [],
    source_hint: "/tmp/rambledesk",
    allow_finish: false,
    final_summary: undefined,
  });
});

test("passes markdown and image attachments through normalization", () => {
  const input = {
    title: "Review",
    what_happened: "A workflow changed.",
    actions: [{ id: "check", instruction: "Check the workflow." }],
    attachments: [
      {
        file_name: "brief.md",
        markdown: "# Brief\n\nReview these notes.",
      },
      {
        file_name: "screenshot.png",
        contents_base64: "iVBORw0KGgoAAAANSUhEUg==",
      },
      {
        file_name: "disk.png",
        path: "/tmp/rambledesk/disk.png",
      },
    ],
  };

  assert.deepEqual(normalizeRequestParams(input, { cwd: "/tmp/rambledesk" }, {}).attachments, [
    { file_name: "brief.md", markdown: "# Brief\n\nReview these notes." },
    { file_name: "screenshot.png", contents_base64: "iVBORw0KGgoAAAANSUhEUg==" },
    { file_name: "disk.png", path: "/tmp/rambledesk/disk.png" },
  ]);
  assert.deepEqual(
    normalizeRequestParams({
      title: "Review",
      what_happened: "A workflow changed.",
      actions: [{ id: "check", instruction: "Check the workflow." }],
    }, { cwd: "/tmp/rambledesk" }, {}).attachments,
    [],
  );
});

test("uses Pi's session manager id instead of grouping requests by cwd", () => {
  const input = {
    host_session_id: "model-supplied-session",
    title: "Review",
    what_happened: "A workflow changed.",
    actions: [{ id: "check", instruction: "Check the workflow." }],
  };

  const normalized = normalizeRequestParams(input, {
    cwd: "/tmp/shared-project",
    sessionManager: { getSessionId: () => "pi-session-uuid" },
  }, {});

  assert.equal(normalized.host_session_id, "pi-session-uuid");
  assert.notEqual(normalized.host_session_id, "pi:/tmp/shared-project");
});

test("uses PI_SESSION_ID when an older Pi context has no session manager", () => {
  const normalized = normalizeRequestParams({
    title: "Review",
    what_happened: "A workflow changed.",
    actions: [{ id: "check", instruction: "Check the workflow." }],
  }, { cwd: "/tmp/shared-project" }, { PI_SESSION_ID: "pi-session-env" });

  assert.equal(normalized.host_session_id, "pi-session-env");
});

test("resolves local API endpoint from explicit API or local server port", () => {
  assert.equal(
    resolveApiBaseUrl({ RAMBLEDESK_LOCAL_API_URL: "http://127.0.0.1:1/api/" }),
    "http://127.0.0.1:1/api",
  );
  assert.equal(resolveApiBaseUrl({ RAMBLEDESK_LOCAL_SERVER_PORT: "3" }), "http://127.0.0.1:3/api");
});

test("health check uses the authenticated local API", async () => {
  const server = http.createServer((request, response) => {
    assert.equal(request.method, "GET");
    assert.equal(request.url, "/api/health");
    assert.equal(request.headers.authorization, "Bearer test-token");
    response.setHeader("content-type", "application/json");
    response.end(JSON.stringify({ ready: true }));
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    assert.equal(await checkHealth({
      RAMBLEDESK_LOCAL_API_URL: `http://127.0.0.1:${server.address().port}/api`,
      RAMBLEDESK_LOCAL_SERVER_TOKEN: "test-token",
    }), true);
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
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

test("blocking wait bypasses Undici's response-header deadline", async () => {
  const server = http.createServer((request, response) => {
    request.resume();
    request.on("end", () => {
      setTimeout(() => {
        response.setHeader("content-type", "application/json");
        response.end(JSON.stringify({
          request_id: "durable-wait-id",
          status: "completed",
          feedback_package: { markdown: "wait survived", attachment_paths: [] },
        }));
      }, 25);
    });
  });
  const originalFetch = globalThis.fetch;
  globalThis.fetch = async () => {
    const error = new TypeError("fetch failed");
    error.cause = { code: "UND_ERR_HEADERS_TIMEOUT" };
    throw error;
  };

  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    const { port } = server.address();
    const result = await postFeedback(
      "wait",
      { request_id: "durable-wait-id" },
      undefined,
      {
        RAMBLEDESK_LOCAL_API_URL: `http://127.0.0.1:${port}/api`,
        RAMBLEDESK_LOCAL_SERVER_TOKEN: "test-token",
      },
    );

    assert.equal(result.request_id, "durable-wait-id");
    assert.equal(result.status, "completed");
  } finally {
    globalThis.fetch = originalFetch;
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

test("request transport errors expose the stable id needed for recovery", async () => {
  const server = http.createServer();
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  await new Promise((resolve) => server.close(resolve));

  await assert.rejects(
    postFeedback(
      "request",
      { request_id: "stable-recovery-id", title: "Review" },
      undefined,
      {
        RAMBLEDESK_LOCAL_API_URL: `http://127.0.0.1:${port}/api`,
        RAMBLEDESK_LOCAL_SERVER_TOKEN: "test-token",
      },
    ),
    (error) => {
      assert.match(error.message, /stable-recovery-id/);
      assert.equal(error.details.request_id, "stable-recovery-id");
      return true;
    },
  );
});

test("puts terminal feedback markdown and attachment paths in model-visible content", () => {
  const result = feedbackToolResult({
    request_id: "019",
    status: "completed",
    feedback_package: {
      markdown: "human feedback",
      attachment_paths: ["\\\\?\\C:\\tmp\\screenshot.png"],
    },
  });

  assert.match(result.content[0].text, /completed/);
  assert.match(result.content[0].text, /human feedback/);
  assert.match(result.content[0].text, /C:\\tmp\\screenshot\.png/);
  assert.doesNotMatch(result.content[0].text, /\\\\\?\\/);
  assert.equal(result.details.feedback_package.markdown, "human feedback");
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
    "resume_ramble_feedback",
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
      {
        cwd: "/tmp/pi-worktree",
        sessionId: "legacy-pi-session",
        sessionManager: { getSessionId: () => "pi-session" },
      },
    );

    assert.equal(calls.length, 2);
    assert.equal(calls[0].url, "/api/feedback/request");
    assert.equal(calls[0].body.host_id, "pi");
    assert.equal(calls[0].body.host_session_id, "pi-session");
    assert.equal(calls[1].url, "/api/feedback/wait");
    assert.deepEqual(calls[1].body, { request_id: "019" });
    assert.match(updates[0].content[0].text, /waiting/);
    assert.match(result.content[0].text, /completed/);
    assert.match(result.content[0].text, /human feedback/);
    assert.match(result.content[0].text, /\/tmp\/screenshot\.png/);
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

test("registers explicit guidance commands and only the intended lifecycle hooks", () => {
  const tools = [];
  const commands = [];
  const lifecycleEvents = [];

  registerRambleDeskPiTools({
    registerTool(tool) { tools.push(tool.name); },
    registerCommand(name) { commands.push(name); },
    on(name) { lifecycleEvents.push(name); },
  });

  assert.deepEqual(tools, [
    "request_ramble_feedback",
    "resume_ramble_feedback",
    "get_ramble_feedback",
  ]);
  assert.deepEqual(commands, ["ramble", "ramble_on", "ramble_off"]);
  assert.deepEqual(lifecycleEvents, ["session_start", "before_agent_start"]);
});

test("interactive startup enables guidance and ramble_off disables it", async () => {
  const commands = new Map();
  const handlers = new Map();
  const notifications = [];
  let healthChecks = 0;
  const server = http.createServer((request, response) => {
    healthChecks += 1;
    assert.equal(request.url, "/api/health");
    response.setHeader("content-type", "application/json");
    response.end(JSON.stringify({ ready: true }));
  });
  const previousApiUrl = process.env.RAMBLEDESK_LOCAL_API_URL;
  const previousToken = process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN;
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    process.env.RAMBLEDESK_LOCAL_API_URL = `http://127.0.0.1:${server.address().port}/api`;
    process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN = "test-token";
    registerRambleDeskPiTools({
      registerTool() {},
      registerCommand(name, command) { commands.set(name, command); },
      on(name, handler) { handlers.set(name, handler); },
    });
    const ctx = {
      mode: "tui",
      hasUI: true,
      ui: { notify(message, level) { notifications.push({ message, level }); } },
    };

    await handlers.get("session_start")({}, { ...ctx, mode: "print", hasUI: false });
    assert.equal(healthChecks, 0);

    await handlers.get("session_start")({}, ctx);
    const enabled = handlers.get("before_agent_start")({ systemPrompt: "base" }, ctx);
    assert.equal(healthChecks, 1);
    assert.match(notifications[0].message, /\/ramble_off/);
    assert.match(enabled.systemPrompt, /RambleDesk feedback mode is enabled/);
    assert.match(enabled.systemPrompt, /Do not create a generic request/);

    await commands.get("ramble_off").handler("", ctx);
    assert.equal(handlers.get("before_agent_start")({ systemPrompt: "base" }, ctx), undefined);

    await commands.get("ramble_on").handler("", ctx);
    assert.equal(healthChecks, 2);
    assert.match(
      handlers.get("before_agent_start")({ systemPrompt: "base" }, ctx).systemPrompt,
      /request_ramble_feedback/,
    );
  } finally {
    if (previousApiUrl === undefined) delete process.env.RAMBLEDESK_LOCAL_API_URL;
    else process.env.RAMBLEDESK_LOCAL_API_URL = previousApiUrl;
    if (previousToken === undefined) delete process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN;
    else process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN = previousToken;
    await new Promise((resolve) => server.close(resolve));
  }
});

test("ramble starts a task-scoped agent turn and uses follow-up delivery while busy", async () => {
  const commands = new Map();
  const sent = [];
  let idle = true;
  const server = http.createServer((_request, response) => {
    response.setHeader("content-type", "application/json");
    response.end(JSON.stringify({ ready: true }));
  });
  const previousApiUrl = process.env.RAMBLEDESK_LOCAL_API_URL;
  const previousToken = process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN;
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    process.env.RAMBLEDESK_LOCAL_API_URL = `http://127.0.0.1:${server.address().port}/api`;
    process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN = "test-token";
    registerRambleDeskPiTools({
      registerTool() {},
      registerCommand(name, command) { commands.set(name, command); },
      on() {},
      sendUserMessage(message, options) { sent.push({ message, options }); },
    });
    const ctx = { mode: "tui", hasUI: false, isIdle: () => idle };

    await commands.get("ramble").handler("  redesign the login page  ", ctx);
    assert.equal(sent.length, 1);
    assert.equal(sent[0].options, undefined);
    assert.doesNotMatch(sent[0].message, /^\/ramble\b/);
    assert.match(sent[0].message, /request_ramble_feedback/);
    assert.match(sent[0].message, /redesign the login page/);
    assert.match(sent[0].message, /unrelated future task/);

    idle = false;
    await commands.get("ramble").handler("review the busy session", ctx);
    assert.deepEqual(sent[1].options, { deliverAs: "followUp" });
  } finally {
    if (previousApiUrl === undefined) delete process.env.RAMBLEDESK_LOCAL_API_URL;
    else process.env.RAMBLEDESK_LOCAL_API_URL = previousApiUrl;
    if (previousToken === undefined) delete process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN;
    else process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN = previousToken;
    await new Promise((resolve) => server.close(resolve));
  }
});

test("ramble without a task starts a kickoff request in RambleDesk", () => {
  const kickoff = buildRambleKickoffMessage("   ");
  assert.match(kickoff, /without providing the task/);
  assert.match(kickoff, /goal, relevant context and constraints, desired output, and completion criteria/);
  assert.match(kickoff, /Do not ask for those details in this chat/);
  assert.match(kickoff, /request_ramble_feedback/);
});

test("ramble does not start an agent turn while RambleDesk is unavailable", async () => {
  const commands = new Map();
  const server = http.createServer();
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  await new Promise((resolve) => server.close(resolve));

  const previousApiUrl = process.env.RAMBLEDESK_LOCAL_API_URL;
  const previousToken = process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN;
  let sent = false;
  try {
    process.env.RAMBLEDESK_LOCAL_API_URL = `http://127.0.0.1:${port}/api`;
    process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN = "test-token";
    registerRambleDeskPiTools({
      registerTool() {},
      registerCommand(name, command) { commands.set(name, command); },
      on() {},
      sendUserMessage() { sent = true; },
    });

    await commands.get("ramble").handler("review the unavailable state", {
      mode: "tui",
      hasUI: false,
      isIdle: () => true,
    });

    assert.equal(sent, false);
  } finally {
    if (previousApiUrl === undefined) delete process.env.RAMBLEDESK_LOCAL_API_URL;
    else process.env.RAMBLEDESK_LOCAL_API_URL = previousApiUrl;
    if (previousToken === undefined) delete process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN;
    else process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN = previousToken;
  }
});

test("resume lazily restores a persisted request without startup hooks", async () => {
  const tools = [];
  const calls = [];
  const server = http.createServer((request, response) => {
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => { body += chunk; });
    request.on("end", () => {
      calls.push({ url: request.url, body: JSON.parse(body) });
      response.setHeader("content-type", "application/json");
      response.end(JSON.stringify({
        request_id: "persisted-request",
        status: "completed",
        resolution: "feedback_submitted",
        feedback_package: { markdown: "Recovered lazily.", attachment_paths: [] },
      }));
    });
  });
  const previousApiUrl = process.env.RAMBLEDESK_LOCAL_API_URL;
  const previousToken = process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN;
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    process.env.RAMBLEDESK_LOCAL_API_URL = `http://127.0.0.1:${server.address().port}/api`;
    process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN = "test-token";
    registerRambleDeskPiTools({
      registerTool(tool) { tools.push(tool); },
      appendEntry() {},
    });
    const result = await tools.find((tool) => tool.name === "resume_ramble_feedback").execute(
      "call-resume",
      {},
      undefined,
      undefined,
      {
        sessionManager: {
          getSessionId: () => "pi-session",
          getEntries: () => [{
            type: "custom",
            customType: "rambledesk-request-state",
            data: { requestId: "persisted-request", phase: "waiting" },
          }],
        },
      },
    );

    assert.deepEqual(calls, [{
      url: "/api/feedback/recover",
      body: { request_id: "persisted-request", host_session_id: "pi-session" },
    }]);
    assert.match(result.content[0].text, /Recovered lazily/);
  } finally {
    if (previousApiUrl === undefined) delete process.env.RAMBLEDESK_LOCAL_API_URL;
    else process.env.RAMBLEDESK_LOCAL_API_URL = previousApiUrl;
    if (previousToken === undefined) delete process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN;
    else process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN = previousToken;
    await new Promise((resolve) => server.close(resolve));
  }
});

test("restores only non-terminal persisted Ramble request state", () => {
  const entry = (phase) => ({ type: "custom", customType: "rambledesk-request-state", data: { requestId: "request-1", phase } });
  assert.equal(restorePendingRequestId([entry("waiting")]), "request-1");
  assert.equal(restorePendingRequestId([entry("waiting"), entry("approved")]), undefined);
  assert.equal(restorePendingRequestId([entry("feedback_submitted")]), undefined);
});

test("approved final summary terminates without another model turn", async () => {
  const tools = [];
  registerRambleDeskPiTools({ registerTool: (tool) => tools.push(tool) });
  const server = http.createServer((request, response) => {
    response.setHeader("content-type", "application/json");
    response.end(JSON.stringify({
      request_id: "019", status: "completed", resolution: "approved",
      allow_finish: true, final_summary: "All requested work is complete.",
    }));
  });
  const previousApiUrl = process.env.RAMBLEDESK_LOCAL_API_URL;
  const previousToken = process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN;
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    process.env.RAMBLEDESK_LOCAL_API_URL = `http://127.0.0.1:${server.address().port}/api`;
    process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN = "test-token";
    const result = await tools.find((tool) => tool.name === "request_ramble_feedback").execute(
      "call-final",
      {
        title: "Final approval",
        what_happened: "The task is complete.",
        actions: [{ id: "approve", instruction: "Review the final summary." }],
        allow_finish: true,
        final_summary: "All requested work is complete.",
      },
      undefined,
      undefined,
      { cwd: "/tmp/project", sessionManager: { getSessionId: () => "pi-session" }, ui: {} },
    );
    assert.equal(result.terminate, true);
    assert.match(result.content[0].text, /approved/);
  } finally {
    if (previousApiUrl === undefined) delete process.env.RAMBLEDESK_LOCAL_API_URL;
    else process.env.RAMBLEDESK_LOCAL_API_URL = previousApiUrl;
    if (previousToken === undefined) delete process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN;
    else process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN = previousToken;
    await new Promise((resolve) => server.close(resolve));
  }
});

test("recovers the package when an idempotent request is already completed", async () => {
  const tools = [];
  registerRambleDeskPiTools({
    registerTool(tool) {
      tools.push(tool);
    },
  });
  const calls = [];
  const server = http.createServer((request, response) => {
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => {
      body += chunk;
    });
    request.on("end", () => {
      calls.push(request.url);
      response.setHeader("content-type", "application/json");
      if (request.url === "/api/feedback/request") {
        response.end(JSON.stringify({ request_id: "019", status: "completed" }));
        return;
      }
      response.end(JSON.stringify({
        request_id: "019",
        status: "completed",
        feedback_package: { markdown: "recovered feedback", attachment_paths: [] },
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
    const requestTool = tools.find((tool) => tool.name === "request_ramble_feedback");
    const result = await requestTool.execute(
      "call-1",
      {
        request_id: "019",
        title: "Review",
        what_happened: "A workflow changed.",
        actions: [{ id: "check", instruction: "Check the workflow." }],
      },
      undefined,
      undefined,
      { cwd: "/tmp/pi-worktree", sessionId: "pi-session" },
    );

    assert.deepEqual(calls, ["/api/feedback/request", "/api/feedback/get"]);
    assert.match(result.content[0].text, /recovered feedback/);
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

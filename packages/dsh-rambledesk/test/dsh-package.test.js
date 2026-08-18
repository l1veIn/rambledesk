import assert from "node:assert/strict";
import http from "node:http";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

import {
  apply,
  defaultTokenPath,
  feedbackToolResult,
  normalizeRequestParams,
  postFeedback,
  registerRambleDshTools,
  registerRambleMode,
  resolveApiBaseUrl,
} from "../index.js";

function fakeTools() {
  const tools = [];
  return {
    register(tool) {
      tools.push(tool);
    },
    list() {
      return tools;
    },
  };
}

function envFor(port, token = "test-token") {
  return {
    RAMBLEDESK_LOCAL_API_URL: `http://127.0.0.1:${port}/api`,
    RAMBLEDESK_LOCAL_SERVER_TOKEN: token,
  };
}

/** Start a mock RambleDesk local server; each request is answered by `handler`. */
function startServer(handler) {
  const server = http.createServer(handler);
  return new Promise((resolve) => server.listen(0, "127.0.0.1", () => resolve(server)));
}

test("normalizes request params with the dsh host identity", async () => {
  const input = {
    title: "Review",
    what_happened: "A workflow changed.",
    actions: [{ id: "check", instruction: "Check the workflow." }],
  };

  const normalized = await normalizeRequestParams(input, {
    hostId: "dsh",
    sourceHint: "C:/work/rambledesk",
    memory: { hostSessionId: "dsh-session-1" },
  });

  assert.equal(normalized.host_id, "dsh");
  assert.equal(normalized.host_session_id, "dsh-session-1");
  assert.equal(normalized.source_hint, "C:/work/rambledesk");
  assert.deepEqual(normalized.context_refs, []);
  assert.deepEqual(normalized.attachments, []);
  assert.equal(normalized.allow_finish, false);
  assert.equal(normalized.final_summary, undefined);
});

test("passes path attachments through normalization", async () => {
  const normalized = await normalizeRequestParams(
    {
      title: "Review",
      what_happened: "A screenshot changed.",
      actions: [{ id: "check", instruction: "Check the screenshot." }],
      attachments: [{ file_name: "disk.png", path: "C:/work/disk.png" }],
    },
    { hostId: "dsh", memory: { hostSessionId: "dsh-session-1" } },
  );

  assert.deepEqual(normalized.attachments, [{ file_name: "disk.png", path: "C:/work/disk.png" }]);
});

test("derives distinct host session ids from distinct dsh sessions", async () => {
  const base = {
    title: "Review",
    what_happened: "A workflow changed.",
    actions: [{ id: "check", instruction: "Check the workflow." }],
  };

  const first = await normalizeRequestParams(base, { hostId: "dsh", sessionId: "session-aaa" });
  const second = await normalizeRequestParams(base, { hostId: "dsh", sessionId: "session-bbb" });
  const explicit = await normalizeRequestParams(
    { ...base, host_session_id: "explicit-session" },
    { hostId: "dsh", sessionId: "session-aaa" },
  );

  assert.equal(first.host_session_id, "dsh-session-aaa");
  assert.equal(second.host_session_id, "dsh-session-bbb");
  assert.notEqual(first.host_session_id, second.host_session_id);
  assert.equal(explicit.host_session_id, "explicit-session");
});

test("the calling dsh session id wins over the persisted machine-wide id", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "dsh-ramble-session-"));
  try {
    const stateFile = path.join(dir, "state.json");
    // Pre-existing installation state: one machine-wide id persisted before
    // the per-session fix.
    await writeFile(stateFile, JSON.stringify({ hostSessionId: "dsh-legacy-machine" }), "utf8");
    const normalized = await normalizeRequestParams({
      title: "T", what_happened: "W", actions: [{ id: "a", instruction: "A" }],
    }, { hostId: "dsh", stateFile, sessionId: "session-live" });

    assert.equal(normalized.host_session_id, "dsh-session-live");
    const persisted = JSON.parse(await readFile(stateFile, "utf8"));
    assert.equal(persisted.hostSessionId, "dsh-legacy-machine");
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("generates and persists a stable host session id across calls", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "dsh-ramble-"));
  try {
    const stateFile = path.join(dir, "state.json");
    const options = { stateFile };
    const first = await normalizeRequestParams({
      title: "T", what_happened: "W", actions: [{ id: "a", instruction: "A" }],
    }, options);
    const second = await normalizeRequestParams({
      title: "T", what_happened: "W", actions: [{ id: "a", instruction: "A" }],
    }, options);

    assert.match(first.host_session_id, /^dsh-/);
    assert.equal(second.host_session_id, first.host_session_id);
    const persisted = JSON.parse(await readFile(stateFile, "utf8"));
    assert.equal(persisted.hostSessionId, first.host_session_id);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("resolves local API endpoint from explicit API or local server port", () => {
  assert.equal(
    resolveApiBaseUrl({ RAMBLEDESK_LOCAL_API_URL: "http://127.0.0.1:1/api/" }),
    "http://127.0.0.1:1/api",
  );
  assert.equal(resolveApiBaseUrl({ RAMBLEDESK_LOCAL_SERVER_PORT: "3" }), "http://127.0.0.1:3/api");
});

test("default token path follows the platform application data directory", () => {
  assert.equal(
    defaultTokenPath({ LOCALAPPDATA: "C:/AppData" }, "win32"),
    path.join("C:/AppData", "RambleDesk", "auth", "local-server.token"),
  );
  assert.equal(
    defaultTokenPath({ XDG_DATA_HOME: "/home/u/.local/share" }, "linux"),
    path.join("/home/u/.local/share", "RambleDesk", "auth", "local-server.token"),
  );
});

test("posts feedback requests with bearer token and dsh host header", async () => {
  const received = [];
  const server = await startServer((request, response) => {
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => { body += chunk; });
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
  try {
    const { port } = server.address();
    const result = await postFeedback("request", { title: "Review" }, undefined, {
      env: envFor(port),
      hostId: "dsh",
    });

    assert.equal(result.request_id, "019");
    assert.equal(received[0].url, "/api/feedback/request");
    assert.equal(received[0].authorization, "Bearer test-token");
    assert.equal(received[0].host, "dsh");
    assert.deepEqual(received[0].body, { title: "Review" });
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
});

test("retries a transient connection failure with the same request id", async () => {
  let attempts = 0;
  const receivedIds = [];
  const server = await startServer((request, response) => {
    attempts += 1;
    if (attempts === 1) {
      request.on("data", () => {});
      request.destroy();
      return;
    }
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => { body += chunk; });
    request.on("end", () => {
      const parsed = JSON.parse(body);
      receivedIds.push(parsed.request_id);
      response.setHeader("content-type", "application/json");
      response.end(JSON.stringify({ request_id: parsed.request_id, status: "waiting" }));
    });
  });
  try {
    const { port } = server.address();
    const result = await postFeedback(
      "request",
      { request_id: "stable-id", title: "Review" },
      undefined,
      { env: envFor(port), hostId: "dsh" },
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
    postFeedback("request", { request_id: "stable-recovery-id", title: "Review" }, undefined, {
      env: envFor(port),
      hostId: "dsh",
    }),
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

  assert.match(result.text, /completed/);
  assert.match(result.text, /human feedback/);
  assert.match(result.text, /C:\\tmp\\screenshot\.png/);
  assert.doesNotMatch(result.text, /\\\\\?\\/);
  assert.equal(result.details.feedback_package.markdown, "human feedback");
});

test("tool result matches the declared output value shape (text + details only)", () => {
  // dsh validates every successful execute() return against output.schema,
  // which declares { text, details } with additionalProperties: false. The
  // MCP-style `content` array shape must never reach the registry.
  const result = feedbackToolResult({
    request_id: "019",
    status: "completed",
    feedback_package: { markdown: "m", attachment_paths: [] },
  });

  assert.deepEqual(Object.keys(result).sort(), ["details", "text"]);
  assert.equal(typeof result.text, "string");
  assert.equal(result.details.request_id, "019");
});

test("registers the four RambleDesk tools on ctx.tools", () => {
  const tools = fakeTools();
  registerRambleDshTools(tools, { stateDir: os.tmpdir() });

  assert.deepEqual(
    tools.list().map((tool) => tool.name),
    ["request_ramble_feedback", "resume_ramble_feedback", "get_ramble_feedback", "cancel_ramble_feedback"],
  );
  for (const tool of tools.list()) {
    assert.equal(typeof tool.execute, "function");
    assert.equal(typeof tool.output.render, "function");
    assert.equal(tool.parameters.type, "object");
    assert.equal(tool.timeoutMs, undefined, `${tool.name} must not declare a deadline`);
  }
});

test("request tool waits inside the tool call and returns the terminal package", async () => {
  const tools = fakeTools();
  const stateDir = await mkdtemp(path.join(os.tmpdir(), "dsh-ramble-state-"));
  try {
    const calls = [];
    const server = await startServer((request, response) => {
      let body = "";
      request.setEncoding("utf8");
      request.on("data", (chunk) => { body += chunk; });
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
    try {
      const { port } = server.address();
      registerRambleDshTools(tools, { env: envFor(port), hostId: "dsh", stateDir });
      const requestTool = tools.list().find((tool) => tool.name === "request_ramble_feedback");
      const result = await requestTool.execute({
        title: "Review",
        what_happened: "A workflow changed.",
        actions: [{ id: "check", instruction: "Check the workflow." }],
      }, { signal: undefined });

      assert.equal(calls.length, 2);
      assert.equal(calls[0].url, "/api/feedback/request");
      assert.equal(calls[0].body.host_id, "dsh");
      assert.match(calls[0].body.host_session_id, /^dsh-/);
      assert.equal(calls[1].url, "/api/feedback/wait");
      assert.deepEqual(calls[1].body, { request_id: "019" });
      assert.match(result.text, /completed/);      assert.match(result.text, /human feedback/);
      assert.match(result.text, /\/tmp\/screenshot\.png/);
      assert.equal(result.details.feedback_package.markdown, "human feedback");
    } finally {
      await new Promise((resolve) => server.close(resolve));
    }
  } finally {
    await rm(stateDir, { recursive: true, force: true });
  }
});

test("recovers the package when an idempotent request is already completed", async () => {
  const tools = fakeTools();
  const calls = [];
  const server = await startServer((request, response) => {
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => { body += chunk; });
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
  try {
    const { port } = server.address();
    registerRambleDshTools(tools, { env: envFor(port), hostId: "dsh", stateDir: os.tmpdir() });
    const requestTool = tools.list().find((tool) => tool.name === "request_ramble_feedback");
    const result = await requestTool.execute({
      request_id: "019",
      title: "Review",
      what_happened: "A workflow changed.",
      actions: [{ id: "check", instruction: "Check the workflow." }],
    }, {});

    assert.deepEqual(calls, ["/api/feedback/request", "/api/feedback/get"]);
    assert.match(result.text, /recovered feedback/);
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
});

test("a completed request never leaks into the next request id", async () => {
  // Regression: persisting the terminal phase used to run through loadState(),
  // whose memory recovery saw the just-cleared pendingRequestId and the still
  // "waiting" persisted phase, restored the finished request id, and the next
  // request_ramble_feedback reused it — failing with REQUEST_CONFLICT until a
  // process restart. Two consecutive requests must use distinct ids.
  const tools = fakeTools();
  const stateDir = await mkdtemp(path.join(os.tmpdir(), "dsh-ramble-leak-"));
  const requestedIds = [];
  const server = await startServer((request, response) => {
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => { body += chunk; });
    request.on("end", () => {
      response.setHeader("content-type", "application/json");
      const parsed = body ? JSON.parse(body) : {};
      if (request.url === "/api/feedback/request") {
        requestedIds.push(parsed.request_id);
        response.end(JSON.stringify({ request_id: parsed.request_id, status: "waiting" }));
        return;
      }
      response.end(JSON.stringify({
        request_id: parsed.request_id,
        status: "completed",
        feedback_package: { markdown: "done", attachment_paths: [] },
      }));
    });
  });
  try {
    const { port } = server.address();
    registerRambleDshTools(tools, { env: envFor(port), hostId: "dsh", stateDir });
    const requestTool = tools.list().find((tool) => tool.name === "request_ramble_feedback");
    const baseArgs = {
      title: "Review",
      what_happened: "A workflow changed.",
      actions: [{ id: "check", instruction: "Check the workflow." }],
    };

    const first = await requestTool.execute(baseArgs, { signal: undefined });
    assert.match(first.text, /completed/);
    const second = await requestTool.execute({ ...baseArgs, wait: false }, { signal: undefined });
    assert.match(second.text, /is waiting/);

    assert.equal(requestedIds.length, 2);
    assert.match(requestedIds[0], /^[0-9a-f]{8}-[0-9a-f]{4}-/);
    assert.match(requestedIds[1], /^[0-9a-f]{8}-[0-9a-f]{4}-/);
    assert.notEqual(requestedIds[1], requestedIds[0], "second request must mint a fresh id");
    const state = JSON.parse(await readFile(path.join(stateDir, "state.json"), "utf8"));
    assert.equal(state.sessions[state.hostSessionId].requestId, requestedIds[1]);
    assert.equal(state.sessions[state.hostSessionId].phase, "waiting");
  } finally {
    await new Promise((resolve) => server.close(resolve));
    await rm(stateDir, { recursive: true, force: true });
  }
});

test("concurrent dsh sessions keep distinct host session ids and pending state", async () => {
  // Regression: the persisted host session id used to be one per machine, so
  // two concurrent sessions shared it; the pending request state also lived in
  // shared top-level fields. Both must be scoped per session.
  const tools = fakeTools();
  const stateDir = await mkdtemp(path.join(os.tmpdir(), "dsh-ramble-concurrent-"));
  const requestedIds = [];
  const server = await startServer((request, response) => {
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => { body += chunk; });
    request.on("end", () => {
      response.setHeader("content-type", "application/json");
      const parsed = body ? JSON.parse(body) : {};
      if (request.url === "/api/feedback/request") {
        requestedIds.push(parsed.request_id);
        response.end(JSON.stringify({ request_id: parsed.request_id, status: "waiting" }));
        return;
      }
      response.end(JSON.stringify({ request_id: parsed.request_id, status: "waiting" }));
    });
  });
  try {
    const { port } = server.address();
    registerRambleDshTools(tools, { env: envFor(port), hostId: "dsh", stateDir });
    const requestTool = tools.list().find((tool) => tool.name === "request_ramble_feedback");
    const baseArgs = {
      title: "Review",
      what_happened: "A workflow changed.",
      actions: [{ id: "check", instruction: "Check the workflow." }],
    };
    const sessionExec = (id) => ({ signal: undefined, agent: { id, session: { header: { id } } } });

    await requestTool.execute({ ...baseArgs, wait: false }, sessionExec("session-alpha"));
    await requestTool.execute({ ...baseArgs, wait: false }, sessionExec("session-beta"));

    assert.equal(requestedIds.length, 2);
    assert.notEqual(requestedIds[1], requestedIds[0], "each session must mint its own id");
    const state = JSON.parse(await readFile(path.join(stateDir, "state.json"), "utf8"));
    assert.equal(state.sessions["dsh-session-alpha"].requestId, requestedIds[0]);
    assert.equal(state.sessions["dsh-session-beta"].requestId, requestedIds[1]);
    assert.equal(state.sessions["dsh-session-alpha"].phase, "waiting");
    assert.equal(state.sessions["dsh-session-beta"].phase, "waiting");
  } finally {
    await new Promise((resolve) => server.close(resolve));
    await rm(stateDir, { recursive: true, force: true });
  }
});

test("resume reconnects to a persisted request and waits for its terminal result", async () => {
  const tools = fakeTools();
  const stateDir = await mkdtemp(path.join(os.tmpdir(), "dsh-ramble-resume-"));
  const calls = [];
  const server = await startServer((request, response) => {
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => { body += chunk; });
    request.on("end", () => {
      calls.push({ url: request.url, body: JSON.parse(body) });
      response.setHeader("content-type", "application/json");
      if (request.url === "/api/feedback/recover") {
        response.end(JSON.stringify({ request_id: "persisted-request", status: "waiting" }));
        return;
      }
      response.end(JSON.stringify({
        request_id: "persisted-request",
        status: "completed",
        feedback_package: { markdown: "Recovered lazily.", attachment_paths: [] },
      }));
    });
  });
  try {
    const { port } = server.address();
    registerRambleDshTools(tools, { env: envFor(port), hostId: "dsh", stateDir });
    const resumeTool = tools.list().find((tool) => tool.name === "resume_ramble_feedback");
    const result = await resumeTool.execute({ request_id: "persisted-request" }, {});

    assert.equal(calls[0].url, "/api/feedback/recover");
    assert.equal(calls[0].body.request_id, "persisted-request");
    assert.match(calls[0].body.host_session_id, /^dsh-/);
    assert.equal(calls[1].url, "/api/feedback/wait");
    assert.match(result.text, /Recovered lazily/);
  } finally {
    await new Promise((resolve) => server.close(resolve));
    await rm(stateDir, { recursive: true, force: true });
  }
});

test("cancel posts an explicit cancellation", async () => {
  const tools = fakeTools();
  const calls = [];
  const server = await startServer((request, response) => {
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => { body += chunk; });
    request.on("end", () => {
      calls.push({ url: request.url, body: JSON.parse(body) });
      response.setHeader("content-type", "application/json");
      response.end(JSON.stringify({ request_id: "019", status: "cancelled" }));
    });
  });
  try {
    const { port } = server.address();
    registerRambleDshTools(tools, { env: envFor(port), hostId: "dsh", stateDir: os.tmpdir() });
    const cancelTool = tools.list().find((tool) => tool.name === "cancel_ramble_feedback");
    const result = await cancelTool.execute({ request_id: "019", reason: "Obsolete." }, {});

    assert.deepEqual(calls[0], {
      url: "/api/feedback/cancel",
      body: { request_id: "019", reason: "Obsolete." },
    });
    assert.match(result.text, /cancelled/);
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
});

test("approved final summary instructs ending the flow without another turn", async () => {
  const tools = fakeTools();
  const server = await startServer((_request, response) => {
    response.setHeader("content-type", "application/json");
    response.end(JSON.stringify({
      request_id: "019",
      status: "completed",
      resolution: "approved",
      allow_finish: true,
      final_summary: "All requested work is complete.",
    }));
  });
  try {
    const { port } = server.address();
    registerRambleDshTools(tools, { env: envFor(port), hostId: "dsh", stateDir: os.tmpdir() });
    const requestTool = tools.list().find((tool) => tool.name === "request_ramble_feedback");
    const result = await requestTool.execute({
      title: "Final approval",
      what_happened: "The task is complete.",
      actions: [{ id: "approve", instruction: "Review the final summary." }],
      allow_finish: true,
      final_summary: "All requested work is complete.",
    }, {});

    assert.match(result.text, /approved/);
    assert.match(result.text, /End the Ramble flow now/);
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
});

test("wait aborts on the execution signal without losing the persisted request", async () => {
  const tools = fakeTools();
  const stateDir = await mkdtemp(path.join(os.tmpdir(), "dsh-ramble-abort-"));
  const controller = new AbortController();
  const server = await startServer((request, response) => {
    if (request.url === "/api/feedback/request") {
      let body = "";
      request.setEncoding("utf8");
      request.on("data", (chunk) => { body += chunk; });
      request.on("end", () => {
        response.setHeader("content-type", "application/json");
        response.end(JSON.stringify({ request_id: JSON.parse(body).request_id, status: "waiting" }));
      });
      return;
    }
    request.on("data", () => {});
    request.on("close", () => {});
    controller.abort(new Error("cancelled by user"));
  });
  try {
    const { port } = server.address();
    registerRambleDshTools(tools, { env: envFor(port), hostId: "dsh", stateDir });
    const requestTool = tools.list().find((tool) => tool.name === "request_ramble_feedback");
    await assert.rejects(
      requestTool.execute({
        title: "Review",
        what_happened: "A workflow changed.",
        actions: [{ id: "check", instruction: "Check the workflow." }],
      }, { signal: controller.signal }),
      /cancelled by user|cancelled/,
    );
    const state = JSON.parse(await readFile(path.join(stateDir, "state.json"), "utf8"));
    // The pending request id is persisted before the request is created, so an
    // aborted wait leaves the durable request recoverable.
    const record = state.sessions[state.hostSessionId];
    assert.match(record.requestId, /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
    assert.equal(record.phase, "waiting");
  } finally {
    await new Promise((resolve) => server.close(resolve));
    await rm(stateDir, { recursive: true, force: true });
  }
});

test("ramble mode is off by default and contributes no prompt text", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "dsh-ramble-mode-"));
  try {
    const contexts = [];
    const commands = [];
    const mode = await registerRambleMode({
      systemPrompt: { context(contribution) { contexts.push(contribution); } },
      commands: { register(definition) { commands.push(definition); } },
    }, { stateDir: dir });

    assert.equal(mode.getMode(), "off");
    assert.equal(contexts[0].name, "rambledesk-mode");
    assert.equal(contexts[0].text({}), "");
    assert.deepEqual(commands.map((command) => command.name), ["ramble", "ramble_off"]);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("config mode on injects the RambleDesk-only constraint per assembly", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "dsh-ramble-mode-on-"));
  try {
    const contexts = [];
    const mode = await registerRambleMode({
      systemPrompt: { context(contribution) { contexts.push(contribution); } },
    }, { stateDir: dir, mode: "on" });

    assert.equal(mode.getMode(), "on");
    const text = contexts[0].text({});
    assert.match(text, /RambleDesk-only mode/);
    assert.match(text, /request_ramble_feedback/);
    assert.match(text, /only communication channel/);

    // The persisted mode survives a re-registration (a new session).
    const second = await registerRambleMode({
      systemPrompt: { context() {} },
    }, { stateDir: dir });
    assert.equal(second.getMode(), "on");
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("ramble command toggles the mode and reports RambleDesk availability", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "dsh-ramble-toggle-"));
  const commands = new Map();
  const server = await startServer((_request, response) => {
    response.setHeader("content-type", "application/json");
    response.end(JSON.stringify({ ready: true }));
  });
  try {
    const { port } = server.address();
    const mode = await registerRambleMode({
      systemPrompt: { context() {} },
      commands: { register(definition) { commands.set(definition.name, definition); } },
    }, { stateDir: dir, env: envFor(port) });

    const enabled = await commands.get("ramble").handler();
    assert.equal(enabled.kind, "success");
    assert.match(enabled.text, /enabled/);
    assert.equal(mode.getMode(), "on");

    const disabled = await commands.get("ramble_off").handler();
    assert.equal(disabled.kind, "success");
    assert.match(disabled.text, /disabled/);
    assert.equal(mode.getMode(), "off");

    const state = JSON.parse(await readFile(path.join(dir, "state.json"), "utf8"));
    assert.equal(state.mode, "off");
  } finally {
    await new Promise((resolve) => server.close(resolve));
    await rm(dir, { recursive: true, force: true });
  }
});

test("ramble command reports an unreachable RambleDesk as an error result", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "dsh-ramble-down-"));
  const commands = new Map();
  const server = http.createServer();
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  await new Promise((resolve) => server.close(resolve));
  try {
    await registerRambleMode({
      systemPrompt: { context() {} },
      commands: { register(definition) { commands.set(definition.name, definition); } },
    }, { stateDir: dir, env: envFor(port) });

    const enabled = await commands.get("ramble").handler();
    assert.equal(enabled.kind, "error");
    assert.match(enabled.text, /not reachable/);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("apply wires tools, ramble mode, and slash commands onto a dsh context", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "dsh-ramble-apply-"));
  try {
    const tools = fakeTools();
    const contexts = [];
    const commands = [];
    const ctx = {
      tools,
      systemPrompt: { context(contribution) { contexts.push(contribution); } },
      commands: { register(definition) { commands.push(definition); } },
    };

    await apply(ctx, { stateDir: dir });

    assert.deepEqual(
      tools.list().map((tool) => tool.name),
      ["request_ramble_feedback", "resume_ramble_feedback", "get_ramble_feedback", "cancel_ramble_feedback"],
    );
    assert.equal(contexts.length, 1);
    assert.equal(contexts[0].name, "rambledesk-mode");
    assert.equal(contexts[0].text({}), ""); // off by default
    assert.deepEqual(commands.map((command) => command.name), ["ramble", "ramble_off"]);
    assert.equal(typeof commands[0].handler, "function");
    assert.equal(typeof commands[1].handler, "function");
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

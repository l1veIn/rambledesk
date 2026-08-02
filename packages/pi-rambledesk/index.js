import { readFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { Type } from "typebox";

const DEFAULT_PORT = 37642;
const HOST_HEADER = "x-rambledesk-host";
const REQUEST_MAX_ATTEMPTS = 3;
const REQUEST_RETRY_DELAY_MS = 300;

const ContextRefSchema = Type.Object({
  label: Type.String({ description: "Short label for the referenced context." }),
  uri: Type.String({ description: "Local file URI, web URL, or other stable reference." }),
});

const ActionSchema = Type.Object({
  id: Type.String({
    description: "Stable action id, lowercase letters/numbers with optional '-' or '_'.",
  }),
  instruction: Type.String({ description: "Concrete action the human should perform." }),
});

export const RequestRambleFeedbackSchema = Type.Object({
  request_id: Type.Optional(
    Type.String({ description: "Optional UUID. Reuse the same id for idempotent retries." }),
  ),
  host_session_id: Type.Optional(
    Type.String({
      description: "Optional Pi host session id. If omitted, the package derives one from Pi context.",
    }),
  ),
  title: Type.String({ description: "Short title shown in the RambleDesk inbox." }),
  what_happened: Type.String({ description: "What changed or what needs feedback." }),
  actions: Type.Array(ActionSchema, {
    minItems: 1,
    description: "Ordered checklist for the human tester.",
  }),
  context_refs: Type.Optional(Type.Array(ContextRefSchema)),
  source_hint: Type.Optional(
    Type.String({ description: "Optional display hint such as cwd or task title." }),
  ),
  wait: Type.Optional(
    Type.Boolean({
      default: true,
      description: "Keep true for Pi's native flow: wait until RambleDesk returns completed/cancelled.",
    }),
  ),
  allow_finish: Type.Optional(
    Type.Boolean({
      default: false,
      description: "Set true only when final_summary is the agent's proposed final answer and the user may approve ending the Ramble flow.",
    }),
  ),
  final_summary: Type.Optional(
    Type.String({ description: "The exact final summary proposed for user approval. Requires allow_finish=true." }),
  ),
});

export const GetRambleFeedbackSchema = Type.Object({
  request_id: Type.String({ description: "RambleDesk request_id returned by request_ramble_feedback." }),
});

export const ResumeRambleFeedbackSchema = Type.Object({
  request_id: Type.Optional(Type.String({ description: "Interrupted request id. Omit to recover the latest request for this exact Pi session." })),
});

export function registerRambleDeskPiTools(pi) {
  let pendingRequestId;
  let serverReady;
  let gateRequired = false;
  let gateReminders = 0;

  pi.on?.("session_start", async (_event, ctx) => {
    pendingRequestId = restorePendingRequestId(ctx.sessionManager?.getEntries?.() ?? []);
    serverReady = await checkHealth().catch(() => false);
    if (pendingRequestId && ctx.hasUI) {
      ctx.ui.setStatus("rambledesk", "RambleDesk · 可恢复中断请求");
    } else if (serverReady && ctx.hasUI) {
      ctx.ui.setStatus("rambledesk", "RambleDesk · 严格反馈已启用");
    }
  });

  pi.on?.("input", (event) => {
    if (event.source === "extension") return;
    gateRequired = true;
    gateReminders = 0;
  });

  pi.on?.("before_agent_start", async (event, ctx) => {
    if (serverReady === undefined) serverReady = await checkHealth().catch(() => false);
    if (!serverReady || !["tui", "rpc"].includes(ctx.mode)) return;
    const recovery = pendingRequestId
      ? ` Request ${pendingRequestId} was interrupted; if the user asks to continue or recover, call resume_ramble_feedback instead of creating a duplicate.`
      : "";
    return {
      systemPrompt: `${event.systemPrompt}\n\nRambleDesk strict feedback gate is active.${recovery} At the beginning of a user task, open a collaboration request before substantial work when clarification is useful. Before voluntarily ending, you must call request_ramble_feedback. Set allow_finish=true and include the exact final_summary only when proposing the completed task for final approval. Feedback submission means continue working; only approval or cancellation permits the flow to end.`,
    };
  });

  pi.on?.("agent_settled", (_event, ctx) => {
    if (!serverReady || !gateRequired || pendingRequestId || !["tui", "rpc"].includes(ctx.mode)) return;
    if (gateReminders >= 2) {
      ctx.ui?.setStatus?.("rambledesk", "RambleDesk · Agent 未执行反馈守门");
      return;
    }
    gateReminders += 1;
    pi.sendMessage?.({
      customType: "rambledesk-gate",
      content: "RambleDesk strict gate blocked this ending. Prepare the exact final summary and call request_ramble_feedback with allow_finish=true, or create a normal feedback request if more input is needed.",
      display: true,
    }, { deliverAs: "followUp", triggerTurn: true });
  });
  pi.registerTool({
    name: "request_ramble_feedback",
    label: "Request RambleDesk Feedback",
    description: `Create a RambleDesk feedback request for the human and wait for the result in this Pi tool call.
Use this instead of MCP when running in Pi. After the tool returns completed, continue the original task using the feedback markdown and attachment paths included in the tool content.
Do not call this tool repeatedly for the same request unless you reuse the same request_id.`,
    promptSnippet:
      "Use request_ramble_feedback when you need the human to test or inspect something in RambleDesk; Pi waits inside the tool call until the feedback package is ready.",
    promptGuidelines: [
      "Prefer request_ramble_feedback over MCP feedback tools in Pi.",
      "Provide a short title, a concrete what_happened summary, and one or more action instructions.",
      "Leave wait as true unless you are intentionally only creating a request for later recovery.",
      "After the tool returns completed, inspect the feedback markdown and any attachment paths included in the tool content before continuing.",
      "Set allow_finish=true only after preparing final_summary. If the human approves it, end without another model turn.",
    ],
    parameters: RequestRambleFeedbackSchema,
    executionMode: "sequential",
    async execute(_toolCallId, params, signal, onUpdate, ctx) {
      const normalized = normalizeRequestParams(params, ctx);
      // A stable per-call request id makes retries idempotent: if the first
      // POST created the request but the response was lost (observed as a
      // transient "fetch failed" on this machine), a retry with the same id
      // reconnects to that request instead of creating a duplicate.
      if (!normalized.request_id) normalized.request_id = crypto.randomUUID();
      pendingRequestId = normalized.request_id;
      appendRequestState(pi, normalized.request_id, "waiting", normalized.host_session_id);
      const created = await postFeedback("request", normalized, signal);
      const terminal = created.status === "completed" || created.status === "cancelled";
      onUpdate?.({
        content: [
          {
            type: "text",
            text: terminal
              ? `RambleDesk request ${created.request_id} is already ${created.status}.`
              : `RambleDesk request ${created.request_id} is ${created.status}; waiting for the human in this Pi tool call.`,
          },
        ],
        details: created,
      });

      const shouldWait = params.wait !== false;
      let result = created;
      if (shouldWait && !terminal) {
        result = await postFeedback("wait", { request_id: created.request_id }, signal);
      } else if (shouldWait && created.status === "completed") {
        // The request endpoint intentionally returns only the request view. An
        // idempotent retry may find an already-completed request, so recover
        // the package before returning it to the model.
        result = await postFeedback("get", { request_id: created.request_id }, signal);
      }
      if (isTerminal(result)) {
        appendRequestState(pi, created.request_id, result.resolution ?? result.status, normalized.host_session_id);
        pendingRequestId = undefined;
        gateRequired = result.resolution === "feedback_submitted";
        gateReminders = 0;
        ctx.ui?.setStatus?.("rambledesk", undefined);
      }
      const output = feedbackToolResult(result);
      if (result.resolution === "approved") output.terminate = true;
      return output;
    },
  });

  pi.registerTool({
    name: "resume_ramble_feedback",
    label: "Resume RambleDesk Feedback",
    description: "Reconnect to an interrupted RambleDesk request for this exact Pi session and wait for its terminal result. Use when the user says continue, resume, or that RambleDesk disconnected.",
    promptSnippet: "Resume the interrupted RambleDesk request instead of creating a duplicate.",
    promptGuidelines: [
      "Use resume_ramble_feedback when the user asks to continue or recover an interrupted RambleDesk flow.",
      "Do not create a new request when resume_ramble_feedback can recover the existing request.",
    ],
    parameters: ResumeRambleFeedbackSchema,
    executionMode: "sequential",
    async execute(_toolCallId, params, signal, onUpdate, ctx) {
      const hostSessionId = readPiSessionId(ctx) ?? process.env.PI_SESSION_ID;
      if (!hostSessionId) throw new Error("Cannot recover RambleDesk feedback without a Pi session id.");
      let recovered = await postFeedback("recover", {
        request_id: params.request_id ?? pendingRequestId,
        host_session_id: hostSessionId,
      }, signal);
      pendingRequestId = recovered.request_id;
      appendRequestState(pi, recovered.request_id, recovered.status, hostSessionId);
      if (!isTerminal(recovered)) {
        onUpdate?.({ content: [{ type: "text", text: `Recovered RambleDesk request ${recovered.request_id}; waiting for the human.` }], details: recovered });
        recovered = await postFeedback("wait", { request_id: recovered.request_id }, signal);
      }
      appendRequestState(pi, recovered.request_id, recovered.resolution ?? recovered.status, hostSessionId);
      pendingRequestId = undefined;
      gateRequired = recovered.resolution === "feedback_submitted";
      gateReminders = 0;
      ctx.ui?.setStatus?.("rambledesk", undefined);
      const output = feedbackToolResult(recovered);
      if (recovered.resolution === "approved") output.terminate = true;
      return output;
    },
  });

  pi.registerTool({
    name: "get_ramble_feedback",
    label: "Get RambleDesk Feedback",
    description:
      "Read a RambleDesk feedback request by request_id. Use for recovery or diagnostics; do not poll waiting requests.",
    promptSnippet: "Use get_ramble_feedback to recover a RambleDesk request by request_id.",
    promptGuidelines: [
      "Use get_ramble_feedback only after resuming or for diagnostics.",
      "Do not poll a waiting request; use request_ramble_feedback with wait=true for Pi's native flow.",
    ],
    parameters: GetRambleFeedbackSchema,
    executionMode: "sequential",
    async execute(_toolCallId, params, signal, _onUpdate, ctx) {
      const result = await postFeedback("get", params, signal);
      if (isTerminal(result)) {
        appendRequestState(pi, result.request_id, result.resolution ?? result.status, readPiSessionId(ctx));
        pendingRequestId = undefined;
        gateRequired = result.resolution === "feedback_submitted";
        gateReminders = 0;
      }
      const output = feedbackToolResult(result);
      if (result.resolution === "approved") output.terminate = true;
      return output;
    },
  });
}

export function normalizeRequestParams(params, ctx = {}, env = process.env) {
  const cwd = typeof ctx.cwd === "string" && ctx.cwd.length > 0 ? ctx.cwd : process.cwd();
  return {
    request_id: params.request_id,
    host_id: "pi",
    host_session_id: firstNonEmpty(
      readPiSessionId(ctx),
      params.host_session_id,
      ctx.sessionId,
      ctx.host_session_id,
      ctx.session?.sessionId,
      ctx.session?.host_session_id,
      env.PI_SESSION_ID,
      `pi:${cwd}`,
    ),
    title: params.title,
    what_happened: params.what_happened,
    actions: params.actions,
    context_refs: params.context_refs ?? [],
    source_hint: firstNonEmpty(params.source_hint, cwd),
    allow_finish: params.allow_finish ?? false,
    final_summary: params.final_summary,
  };
}

export async function checkHealth(env = process.env, signal) {
  const baseUrl = resolveApiBaseUrl(env);
  const token = await resolveAccessToken(env);
  const response = await fetch(`${baseUrl}/health`, {
    method: "GET",
    headers: { Authorization: `Bearer ${token}`, [HOST_HEADER]: "pi" },
    signal: signal ?? AbortSignal.timeout(500),
  });
  if (!response.ok) return false;
  const body = await response.json();
  return body?.ready === true;
}

export async function postFeedback(action, payload, signal, env = process.env) {
  const baseUrl = resolveApiBaseUrl(env);
  const token = await resolveAccessToken(env);
  const body = await fetchWithRetry(action, baseUrl, token, payload, signal, env, 1);
  return body;
}

async function fetchWithRetry(action, baseUrl, token, payload, signal, env, attempt) {
  let response;
  try {
    response = await fetch(`${baseUrl}/feedback/${action}`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
        [HOST_HEADER]: "pi",
      },
      body: JSON.stringify(payload),
      signal,
    });
  } catch (cause) {
    if (attempt < REQUEST_MAX_ATTEMPTS && !signal?.aborted) {
      await sleep(REQUEST_RETRY_DELAY_MS * attempt);
      return fetchWithRetry(action, baseUrl, token, payload, signal, env, attempt + 1);
    }
    const error = new Error(`RambleDesk unreachable: ${cause?.message ?? cause}`);
    error.details = { action, attempt };
    if (action === "request") {
      error.message +=
        " The request may still have been created; check the RambleDesk inbox before retrying manually, or reuse the same request_id.";
    }
    throw error;
  }
  const text = await response.text();
  const body = text.length > 0 ? JSON.parse(text) : {};
  if (!response.ok) {
    const message = body?.message || response.statusText || `HTTP ${response.status}`;
    const error = new Error(`RambleDesk ${body?.code || response.status}: ${message}`);
    error.details = body;
    throw error;
  }
  return body;
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export function feedbackToolResult(result) {
  const status = result?.status;
  const requestId = result?.request_id;
  const text = result?.resolution === "approved"
    ? `RambleDesk final summary ${requestId} was approved by the human. End the Ramble flow now without another model response or feedback request.`
    : status === "completed"
    ? completedFeedbackText(result, requestId)
    : status === "cancelled"
      ? `RambleDesk feedback request ${requestId} was cancelled. Treat this as terminal and continue or stop accordingly.`
      : `RambleDesk feedback request ${requestId} is ${status}. Do not poll; wait for a resume signal or call get_ramble_feedback later.`;
  return {
    content: [{ type: "text", text }],
    details: result,
  };
}

function completedFeedbackText(result, requestId) {
  const feedbackPackage = result?.feedback_package;
  const markdown = typeof feedbackPackage?.markdown === "string"
    ? feedbackPackage.markdown
    : "";
  const attachmentPaths = Array.isArray(feedbackPackage?.attachment_paths)
    ? feedbackPackage.attachment_paths
      .filter((value) => typeof value === "string" && value.length > 0)
      .map(normalizeDisplayPath)
    : [];
  const sections = [`RambleDesk feedback request ${requestId} is completed.`];
  sections.push(markdown.length > 0
    ? `Feedback markdown:\n\n--- BEGIN RAMBLEDESK FEEDBACK ---\n${markdown}\n--- END RAMBLEDESK FEEDBACK ---`
    : "The completed response did not include feedback_package.markdown. Call get_ramble_feedback once to recover it.");
  sections.push(attachmentPaths.length > 0
    ? `Attachment paths:\n${attachmentPaths.map((value) => `- ${value}`).join("\n")}`
    : "Attachment paths: none.");
  return sections.join("\n\n");
}

function isTerminal(result) {
  return result?.status === "completed" || result?.status === "cancelled";
}

function appendRequestState(pi, requestId, phase, hostSessionId) {
  pi.appendEntry?.("rambledesk-request-state", {
    requestId,
    hostSessionId,
    phase,
    timestamp: Date.now(),
  });
}

export function restorePendingRequestId(entries) {
  const states = entries
    .filter((entry) => entry?.type === "custom" && entry.customType === "rambledesk-request-state")
    .map((entry) => entry.data)
    .filter((data) => typeof data?.requestId === "string");
  const latest = states.at(-1);
  if (!latest || ["completed", "cancelled", "approved", "feedback_submitted"].includes(latest.phase)) return undefined;
  return latest.requestId;
}

function normalizeDisplayPath(value) {
  const verbatimUncPrefix = "\\\\?\\UNC\\";
  const verbatimPrefix = "\\\\?\\";
  if (value.startsWith(verbatimUncPrefix)) {
    return `\\\\${value.slice(verbatimUncPrefix.length)}`;
  }
  if (value.startsWith(verbatimPrefix)) {
    return value.slice(verbatimPrefix.length);
  }
  return value;
}

export function resolveApiBaseUrl(env = process.env) {
  const explicit = firstNonEmpty(env.RAMBLEDESK_LOCAL_API_URL);
  if (explicit) return stripTrailingSlash(explicit);

  const port = firstNonEmpty(env.RAMBLEDESK_LOCAL_SERVER_PORT, `${DEFAULT_PORT}`);
  return `http://127.0.0.1:${port}/api`;
}

export async function resolveAccessToken(env = process.env) {
  const explicit = firstNonEmpty(env.RAMBLEDESK_LOCAL_SERVER_TOKEN);
  if (explicit) return explicit;
  const tokenPath = firstNonEmpty(env.RAMBLEDESK_LOCAL_SERVER_TOKEN_FILE, defaultTokenPath(env));
  return (await readFile(tokenPath, "utf8")).trim();
}

export function defaultTokenPath(env = process.env, platform = process.platform) {
  if (platform === "darwin") {
    return path.join(os.homedir(), "Library", "Application Support", "RambleDesk", "auth", "local-server.token");
  }
  if (platform === "win32") {
    const root = firstNonEmpty(env.LOCALAPPDATA, path.join(os.homedir(), "AppData", "Local"));
    return path.join(root, "RambleDesk", "auth", "local-server.token");
  }
  const root = firstNonEmpty(env.XDG_DATA_HOME, path.join(os.homedir(), ".local", "share"));
  return path.join(root, "RambleDesk", "auth", "local-server.token");
}

function readPiSessionId(ctx) {
  const getSessionId = ctx.sessionManager?.getSessionId;
  if (typeof getSessionId !== "function") return undefined;
  try {
    return getSessionId.call(ctx.sessionManager);
  } catch {
    return undefined;
  }
}

function firstNonEmpty(...values) {
  for (const value of values) {
    if (typeof value === "string" && value.trim().length > 0) return value.trim();
  }
  return undefined;
}

function stripTrailingSlash(value) {
  return value.replace(/\/+$/, "");
}

export default function rambledeskPiPackage(pi) {
  registerRambleDeskPiTools(pi);
}

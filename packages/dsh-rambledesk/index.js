// RambleDesk native adapter for the DeepSeek Harness (dsh).
//
// A Cordis plugin mirroring `packages/pi-rambledesk`: it talks to the
// authenticated loopback JSON API directly and waits inside the dsh tool call,
// so no post-submit continuation strategy is needed. The tool call blocks
// until the human submits or cancels in RambleDesk; the request stays
// durable server-side and `resume_ramble_feedback` reconnects after an
// interrupt or restart.
//
// No npm dependencies: tools are registered as plain objects on `ctx.tools`
// (the dsh-tools registry accepts standard JSON Schema definitions), and the
// token is read from the RambleDesk local server token file on every call.

import { readFile, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import crypto from "node:crypto";

const DEFAULT_PORT = 37642;
const HOST_HEADER = "x-rambledesk-host";
const REQUEST_MAX_ATTEMPTS = 3;
const REQUEST_RETRY_DELAY_MS = 300;
const NON_WAIT_TIMEOUT_MS = 10_000;
const HEALTH_CHECK_TIMEOUT_MS = 500;
const TERMINAL_PHASES = ["completed", "cancelled", "approved", "feedback_submitted"];

/// Text injected into the system prompt while ramble mode is on. It is
/// evaluated per assembly, so flipping the mode flips the model's behaviour on
/// the next turn without a plugin reload.
const RAMBLE_MODE_TEXT = `## RambleDesk-only mode
You are in RambleDesk-only mode: your only communication channel with the human is RambleDesk (the desktop workbench). The human will not read this chat.
- When you need the human to operate, review, verify, decide, or give feedback: call request_ramble_feedback (keep wait=true; the tool call blocks until the human submits) and then end the turn.
- The human performs the actions, issues further instructions, and submits a feedback package in RambleDesk; when the tool returns completed, implement the package item by item.
- Do not ask questions, wait, or solicit input in this chat. Report progress here only briefly.
- After an interrupt or restart, reconnect with resume_ramble_feedback instead of creating a duplicate request; when the human explicitly gives up, call cancel_ramble_feedback.
- Never create a generic request merely because a task started or the agent is about to finish.`;

const RAMBLE_MODE_NAME = "rambledesk-mode";

// #region schema helpers

const stringField = (description) => ({ type: "string", description });

const ContextRefSchema = {
  type: "object",
  properties: {
    label: stringField("Short label for the referenced context."),
    uri: stringField("Local file URI, web URL, or other stable reference."),
  },
  required: ["label", "uri"],
};

const ActionSchema = {
  type: "object",
  properties: {
    id: stringField("Stable action id, lowercase letters/numbers with optional '-' or '_'."),
    instruction: stringField("Concrete action the human should perform."),
  },
  required: ["id", "instruction"],
};

const RequestAttachmentSchema = {
  type: "object",
  properties: {
    file_name: stringField("Attachment file name. Use a .md or .markdown extension for Markdown documents."),
    markdown: stringField("Markdown document contents (text/markdown). Requires a .md or .markdown file_name. Mutually exclusive with contents_base64."),
    contents_base64: stringField("Base64-encoded image contents. Must decode to a PNG, JPEG, GIF, or WebP image. Mutually exclusive with markdown."),
  },
  required: ["file_name"],
};

const requestParamsSchema = {
  type: "object",
  properties: {
    request_id: stringField("Optional UUID. Reuse the same id for idempotent retries."),
    host_session_id: stringField("Optional dsh host session id. If omitted, the plugin derives and persists one for this machine."),
    title: stringField("Short title shown in the RambleDesk inbox."),
    what_happened: stringField("What changed or what needs feedback."),
    actions: {
      type: "array",
      minItems: 1,
      description: "Ordered checklist for the human tester.",
      items: ActionSchema,
    },
    context_refs: { type: "array", description: "Optional readable context references.", items: ContextRefSchema },
    attachments: {
      type: "array",
      description: "Markdown documents and images the human should review with this request.",
      items: RequestAttachmentSchema,
    },
    source_hint: stringField("Optional display hint such as task title or workspace path."),
    wait: {
      type: "boolean",
      default: true,
      description: "Keep true for dsh's native flow: wait until RambleDesk returns completed/cancelled inside this tool call.",
    },
    allow_finish: {
      type: "boolean",
      default: false,
      description: "Set true only when final_summary is the agent's proposed final answer and the user may approve ending the Ramble flow.",
    },
    final_summary: stringField("The exact final summary proposed for user approval. Requires allow_finish=true."),
  },
  required: ["title", "what_happened", "actions"],
};

// #endregion

// #region request state (per-plugin, persisted lazily)

export function stateFilePath(options, fallbackUrl = import.meta.url) {
  if (options.stateFile) return options.stateFile;
  if (options.stateDir) return path.join(options.stateDir, "state.json");
  return fileURLToPath(new URL("state.json", fallbackUrl));
}

export async function readPersistedState(stateFile) {
  try {
    const parsed = JSON.parse(await readFile(stateFile, "utf8"));
    if (typeof parsed === "object" && parsed !== null) return parsed;
    return {};
  } catch {
    return {};
  }
}

export async function writePersistedState(stateFile, state) {
  try {
    await writeFile(stateFile, `${JSON.stringify(state, null, 2)}\n`, "utf8");
  } catch {
    // Persistence is best-effort: an unwritable plugin directory must not
    // break feedback requests. The pending request id still lives in memory.
  }
}

// #endregion

// #region config resolution

export function resolveApiBaseUrl(env = process.env) {
  const explicit = firstNonEmpty(env.RAMBLEDESK_LOCAL_API_URL);
  if (explicit) return stripTrailingSlash(explicit);
  const port = firstNonEmpty(env.RAMBLEDESK_LOCAL_SERVER_PORT, `${DEFAULT_PORT}`);
  return `http://127.0.0.1:${port}/api`;
}

export async function resolveAccessToken(env = process.env, options = {}) {
  const explicit = firstNonEmpty(env.RAMBLEDESK_LOCAL_SERVER_TOKEN);
  if (explicit) return explicit;
  const tokenPath = firstNonEmpty(env.RAMBLEDESK_LOCAL_SERVER_TOKEN_FILE, options.tokenFile, defaultTokenPath(env));
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

// #endregion

// #region transport

export async function postFeedback(action, payload, signal, options) {
  const baseUrl = options.apiBaseUrl ?? resolveApiBaseUrl(options.env ?? process.env);
  const token = await resolveAccessToken(options.env ?? process.env, options);
  const timeoutMs = action === "wait" ? undefined : NON_WAIT_TIMEOUT_MS;
  return fetchWithRetry(action, baseUrl, token, payload, signal, options, 1, timeoutMs);
}

async function fetchWithRetry(action, baseUrl, token, payload, signal, options, attempt, timeoutMs) {
  const requestSignal = combineSignals(signal, timeoutMs);
  let response;
  try {
    response = await fetch(`${baseUrl}/feedback/${action}`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
        [HOST_HEADER]: options.hostId ?? "dsh",
      },
      body: JSON.stringify(payload),
      signal: requestSignal,
    });
  } catch (cause) {
    if (attempt < REQUEST_MAX_ATTEMPTS && !signal?.aborted) {
      await sleep(REQUEST_RETRY_DELAY_MS * attempt);
      return fetchWithRetry(action, baseUrl, token, payload, signal, options, attempt + 1, timeoutMs);
    }
    const error = new Error(`RambleDesk unreachable: ${cause?.message ?? cause}`);
    error.details = { action, attempt, request_id: payload?.request_id };
    if (action === "request") {
      error.message += payload?.request_id
        ? ` The request may still have been created. Resume or retry with request_id ${payload.request_id}; do not create a replacement request.`
        : " The request may still have been created; resume the pending RambleDesk request instead of creating a replacement.";
    }
    throw error;
  }
  const text = await response.text();
  const body = parseJsonBody(text, action, response.status);
  if (!response.ok) {
    const message = body?.message || response.statusText || `HTTP ${response.status}`;
    const error = new Error(`RambleDesk ${body?.code || response.status}: ${message}`);
    error.details = body;
    throw error;
  }
  return body;
}

function parseJsonBody(text, action, status) {
  if (text.length === 0) return {};
  try {
    return JSON.parse(text);
  } catch {
    throw new Error(
      `RambleDesk returned a non-JSON response for ${action} (HTTP ${status}): ${text.slice(0, 200)}`,
    );
  }
}

function combineSignals(signal, timeoutMs) {
  if (!signal) return timeoutMs === undefined ? undefined : AbortSignal.timeout(timeoutMs);
  if (timeoutMs === undefined) return signal;
  return AbortSignal.any([signal, AbortSignal.timeout(timeoutMs)]);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// #endregion

// #region request normalization

export async function normalizeRequestParams(params, options = {}) {
  const hostId = options.hostId ?? "dsh";
  const sessionId = await resolveHostSessionId(params, options);
  return {
    request_id: params.request_id,
    host_id: hostId,
    host_session_id: sessionId,
    title: params.title,
    what_happened: params.what_happened,
    actions: params.actions,
    context_refs: params.context_refs ?? [],
    attachments: params.attachments ?? [],
    source_hint: firstNonEmpty(params.source_hint, options.sourceHint),
    allow_finish: params.allow_finish ?? false,
    final_summary: params.final_summary,
  };
}

async function resolveHostSessionId(params, options) {
  if (firstNonEmpty(params.host_session_id)) return params.host_session_id.trim();
  if (options.memory?.hostSessionId) return options.memory.hostSessionId;
  const persisted = options.stateFile ? await readPersistedState(options.stateFile) : {};
  if (typeof persisted.hostSessionId === "string" && persisted.hostSessionId.length > 0) {
    if (options.memory) options.memory.hostSessionId = persisted.hostSessionId;
    return persisted.hostSessionId;
  }
  const generated = `dsh-${crypto.randomUUID()}`;
  if (options.memory) options.memory.hostSessionId = generated;
  if (options.stateFile) {
    await writePersistedState(options.stateFile, { ...persisted, hostSessionId: generated });
  }
  return generated;
}

// #endregion

// #region result rendering

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
    text,
    details: result,
  };
}

function completedFeedbackText(result, requestId) {
  const feedbackPackage = result?.feedback_package;
  const markdown = typeof feedbackPackage?.markdown === "string" ? feedbackPackage.markdown : "";
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

// #endregion

// #region tool registration

export function registerRambleDshTools(tools, options = {}) {
  const memory = { hostSessionId: undefined, pendingRequestId: undefined };
  const stateFile = stateFilePath(options);

  async function loadState() {
    const persisted = await readPersistedState(stateFile);
    if (!memory.hostSessionId && typeof persisted.hostSessionId === "string") {
      memory.hostSessionId = persisted.hostSessionId;
    }
    if (!memory.pendingRequestId && !TERMINAL_PHASES.includes(persisted.phase)) {
      memory.pendingRequestId = persisted.requestId;
    }
    return persisted;
  }

  async function persistRequest(phase, requestId, hostSessionId) {
    // Read the file directly, never through loadState(): loadState's memory
    // recovery would see memory.pendingRequestId === undefined (just cleared
    // for a terminal phase) and the persisted phase still "waiting", then
    // restore the just-finished request id as pending. The next
    // request_ramble_feedback would reuse a completed request id and fail
    // with REQUEST_CONFLICT until the process restarts.
    const persisted = await readPersistedState(stateFile);
    await writePersistedState(stateFile, {
      ...persisted,
      requestId,
      hostSessionId,
      phase,
      timestamp: Date.now(),
    });
  }

  function clearPending(requestId, hostSessionId) {
    if (memory.pendingRequestId === requestId) memory.pendingRequestId = undefined;
    void persistRequest("completed", requestId, hostSessionId);
  }

  const requestTool = {
    name: "request_ramble_feedback",
    description: `Create a RambleDesk feedback request for the human and wait for the result inside this dsh tool call.
Use this instead of an MCP bridge when running in dsh. After the tool returns completed, continue the original task using the feedback markdown and attachment paths included in the tool content.
Optional attachments give the human review artifacts: attachments[].markdown with a .md/.markdown file_name for a Markdown document, or attachments[].contents_base64 for a PNG/JPEG/GIF/WebP image.
Do not call this tool repeatedly for the same request unless you reuse the same request_id.`,
    parameters: requestParamsSchema,
    output: toolOutput(),
    async execute(args, exec) {
      const signal = exec?.signal;
      const normalized = await normalizeRequestParams(args, {
        ...options,
        memory,
        stateFile,
        sourceHint: exec?.cwd,
      });
      if (!normalized.request_id) {
        normalized.request_id = memory.pendingRequestId ?? crypto.randomUUID();
      }
      memory.pendingRequestId = normalized.request_id;
      await persistRequest("waiting", normalized.request_id, normalized.host_session_id);
      const created = await postFeedback("request", normalized, signal, options);
      const terminal = created.status === "completed" || created.status === "cancelled";
      let result = created;
      if (args.wait !== false && !terminal) {
        result = await postFeedback("wait", { request_id: created.request_id }, signal, options);
      } else if (args.wait !== false && created.status === "completed") {
        // The request endpoint intentionally returns only the request view. An
        // idempotent retry may find an already-completed request, so recover
        // the package before returning it to the model.
        result = await postFeedback("get", { request_id: created.request_id }, signal, options);
      }
      if (isTerminal(result)) {
        memory.pendingRequestId = undefined;
        await persistRequest(result.resolution ?? result.status, result.request_id, normalized.host_session_id);
      }
      return feedbackToolResult(result);
    },
  };

  const resumeTool = {
    name: "resume_ramble_feedback",
    description: "Reconnect to an interrupted RambleDesk request for this dsh session and wait for its terminal result. Use when the user says continue, resume, or that RambleDesk disconnected.",
    parameters: {
      type: "object",
      properties: {
        request_id: stringField("Interrupted request id. Omit to recover the latest pending request for this dsh session."),
      },
    },
    output: toolOutput(),
    async execute(args, exec) {
      const signal = exec?.signal;
      const persisted = await loadState();
      const hostSessionId = await resolveHostSessionId(args, { ...options, memory, stateFile });
      const restored = firstNonEmpty(args.request_id, memory.pendingRequestId, persisted.requestId);
      if (!restored) {
        throw new Error("Cannot recover RambleDesk feedback without a request id; pass request_id or run request_ramble_feedback first.");
      }
      const recovered = await postFeedback("recover", {
        request_id: restored,
        host_session_id: hostSessionId,
      }, signal, options);
      memory.pendingRequestId = recovered.request_id;
      await persistRequest(recovered.status, recovered.request_id, hostSessionId);
      let result = recovered;
      if (!isTerminal(recovered)) {
        result = await postFeedback("wait", { request_id: recovered.request_id }, signal, options);
      }
      if (isTerminal(result)) {
        memory.pendingRequestId = undefined;
        await persistRequest(result.resolution ?? result.status, result.request_id, hostSessionId);
      }
      return feedbackToolResult(result);
    },
  };

  const getTool = {
    name: "get_ramble_feedback",
    description: "Read a RambleDesk feedback request by request_id. Use for recovery or diagnostics; do not poll waiting requests.",
    parameters: {
      type: "object",
      properties: {
        request_id: stringField("RambleDesk request_id returned by request_ramble_feedback."),
      },
      required: ["request_id"],
    },
    output: toolOutput(),
    async execute(args, exec) {
      const signal = exec?.signal;
      const result = await postFeedback("get", { request_id: args.request_id }, signal, options);
      if (isTerminal(result)) {
        memory.pendingRequestId = undefined;
      }
      return feedbackToolResult(result);
    },
  };

  const cancelTool = {
    name: "cancel_ramble_feedback",
    description: "Cancel a waiting or in-progress RambleDesk feedback request. Repeated cancellation preserves the first cancellation.",
    parameters: {
      type: "object",
      properties: {
        request_id: stringField("RambleDesk request_id returned by request_ramble_feedback."),
        reason: stringField("Optional reason shown to the human and persisted with the cancellation."),
      },
      required: ["request_id"],
    },
    output: toolOutput(),
    async execute(args, exec) {
      const signal = exec?.signal;
      const result = await postFeedback("cancel", { request_id: args.request_id, ...(args.reason ? { reason: args.reason } : {}) }, signal, options);
      if (isTerminal(result)) {
        memory.pendingRequestId = undefined;
      }
      return feedbackToolResult(result);
    },
  };

  for (const tool of [requestTool, resumeTool, getTool, cancelTool]) {
    tools.register(tool);
  }
  return { memory };
}

function toolOutput() {
  return {
    schema: {
      type: "object",
      properties: {
        text: { type: "string", description: "Model-facing feedback result text." },
        details: {},
      },
      required: ["text"],
      additionalProperties: false,
    },
    render(_args, value) {
      return [{ type: "text", text: value.text }];
    },
  };
}

function isTerminal(result) {
  return result?.status === "completed" || result?.status === "cancelled";
}

// #endregion

// #region ramble mode

/**
 * The persistent ramble-mode switch: when on, every session's system prompt
 * carries the RambleDesk-only constraint. The mode is persisted in the same
 * `state.json` as request state, so a new dsh session inherits it.
 *
 * `services` is a minimal object exposing the optional dsh services:
 * - `systemPrompt.context(contribution)` registers the dynamic prompt context
 *   (evaluated per assembly; empty text contributes nothing),
 * - `commands.register(definition)` registers `/ramble` and `/ramble_off`.
 *
 * Returns `{ getMode, setMode }` for tests and command handlers.
 */
export async function registerRambleMode(services, options = {}) {
  const stateFile = options.stateFile ?? stateFilePath(options);
  const persisted = await readPersistedState(stateFile);
  const mode = persisted.mode === "on" || persisted.mode === "off"
    ? persisted.mode
    : options.mode === "on"
      ? "on"
      : "off";
  let current = mode;

  async function persistMode(next) {
    const state = await readPersistedState(stateFile);
    await writePersistedState(stateFile, { ...state, mode: next });
  }

  // A config-derived initial "on" is written once so later sessions (and
  // restarts) inherit the mode from state rather than needing the config.
  if (current === "on" && persisted.mode !== "on") {
    await persistMode("on");
  }

  if (services.systemPrompt) {
    services.systemPrompt.context({
      name: RAMBLE_MODE_NAME,
      order: options.modeOrder ?? 100,
      text: () => (current === "on" ? RAMBLE_MODE_TEXT : ""),
    });
  }

  if (services.commands) {
    services.commands.register({
      name: "ramble",
      description: "Enable RambleDesk-only mode for every dsh session: the agent communicates with the human only through RambleDesk feedback requests.",
      handler: async () => {
        current = "on";
        await persistMode(current);
        const available = await checkHealth(options).catch(() => false);
        return {
          kind: available ? "success" : "error",
          text: available
            ? "RambleDesk-only mode enabled. The agent now communicates with you only through RambleDesk; the chat is progress-only."
            : "RambleDesk-only mode enabled, but the RambleDesk app is not reachable. Start RambleDesk before the agent creates a feedback request.",
        };
      },
    });
    services.commands.register({
      name: "ramble_off",
      description: "Disable RambleDesk-only mode; the agent returns to normal chat communication.",
      handler: async () => {
        current = "off";
        await persistMode(current);
        return {
          kind: "success",
          text: "RambleDesk-only mode disabled. The agent will communicate normally in this chat.",
        };
      },
    });
  }

  return {
    getMode: () => current,
    setMode: async (next) => {
      current = next === "on" ? "on" : "off";
      await persistMode(current);
    },
  };
}

/** One bounded local health probe against the RambleDesk local server. */
export async function checkHealth(options = {}, env = process.env) {
  const baseUrl = options.apiBaseUrl ?? resolveApiBaseUrl(options.env ?? env);
  const token = await resolveAccessToken(options.env ?? env, options);
  const response = await fetch(`${baseUrl}/health`, {
    method: "GET",
    headers: {
      Authorization: `Bearer ${token}`,
      [HOST_HEADER]: options.hostId ?? "dsh",
    },
    signal: AbortSignal.timeout(HEALTH_CHECK_TIMEOUT_MS),
  });
  if (!response.ok) return false;
  const body = await response.json();
  return body?.ready === true;
}

// #endregion

// #region cordis plugin

const name = "rambledesk";
const inject = ["tools"];

async function apply(ctx, config = {}) {
  const options = {
    hostId: config.hostId ?? "dsh",
    apiBaseUrl: config.apiBaseUrl,
    tokenFile: config.tokenFile,
    stateFile: config.stateFile,
    stateDir: config.stateDir,
  };
  registerRambleDshTools(ctx.tools, options);
  await registerRambleMode({
    systemPrompt: ctx.get("systemPrompt"),
    commands: ctx.get("commands"),
  }, {
    ...options,
    mode: config.mode,
    modeOrder: config.modeOrder,
  });
}

export { apply, inject, name };
export default { apply, inject, name };

// #endregion

// #region helpers

function firstNonEmpty(...values) {
  for (const value of values) {
    if (typeof value === "string" && value.trim().length > 0) return value.trim();
  }
  return undefined;
}

function stripTrailingSlash(value) {
  return value.replace(/\/+$/, "");
}

// #endregion

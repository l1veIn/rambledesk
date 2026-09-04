// Managed ACP entry point: the application alone owns human-feedback delivery.
// Pi receives the existing private MCP tools as native extension tools; this
// extension never waits for the human or sends an extra continuation message.
import { randomUUID } from "node:crypto";
import { ManagedFeedbackClient } from "./managed-client.mjs";

const STATE = "rambledesk-managed-request-state";
const NAMES = ["request_feedback", "get_feedback", "recover_feedback"];
const terminal = (result) => ["completed", "cancelled"].includes(result?.status);

function restoredRequest(ctx) {
  const entries = ctx?.sessionManager?.getEntries?.() ?? [];
  const record = entries.filter((entry) => entry?.type === "custom" && entry.customType === STATE).at(-1)?.data;
  return record?.status === "waiting" && typeof record.request_id === "string" ? record.request_id : undefined;
}

export async function registerManagedRambleDeskTools(pi, env = process.env) {
  const client = new ManagedFeedbackClient(env);
  const info = await client.initialize();
  const tools = await client.tools();
  if (!Array.isArray(tools) || NAMES.some((name) => !tools.some((tool) => tool.name === name))) throw new Error("The managed feedback binding does not expose the required private tools.");
  let pending;
  pi.on?.("before_agent_start", (event) => ({ systemPrompt: `${event.systemPrompt}\n\n${info.instructions ?? "After request_feedback, end this turn. RambleDesk continues the same Agent session after human feedback."}` }));
  for (const name of NAMES) {
    const tool = tools.find((tool) => tool.name === name);
    pi.registerTool({
      name,
      label: tool.title ?? name,
      description: tool.description,
      parameters: tool.inputSchema,
      executionMode: "sequential",
      async execute(_toolCallId, params, signal, _onUpdate, ctx) {
        const args = { ...params };
        // Transport-selected identity cannot be overridden through native tool
        // params either. The private HTTP server independently enforces this.
        for (const key of ["host_id", "host_session_id", "managed_session_id", "wait"]) delete args[key];
        if (name === "request_feedback") {
          args.request_id ??= pending ?? restoredRequest(ctx) ?? randomUUID();
          pending = args.request_id;
          pi.appendEntry?.(STATE, { request_id: pending, status: "waiting" });
        } else if (name === "recover_feedback") {
          args.request_id ??= pending ?? restoredRequest(ctx);
        }
        const result = await client.call(name, args, signal);
        const body = result.structuredContent;
        if (!result.isError && body?.request_id) {
          const status = terminal(body) ? body.status : "waiting";
          pi.appendEntry?.(STATE, { request_id: body.request_id, status });
          pending = status === "waiting" ? body.request_id : undefined;
        }
        // Preserve the same feedback markdown and attachment paths returned by
        // MCP. No Pi-specific approval/termination or double continuation.
        return { content: result.content, details: body ?? result, isError: result.isError };
      },
    });
  }
}

export default registerManagedRambleDeskTools;

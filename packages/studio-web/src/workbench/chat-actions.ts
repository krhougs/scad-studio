import type {
  CadQueryExportFormat,
  AgentOperationLevel,
  AgentPlanProposedEvent,
} from "@budn/app-server-protocol";
import type { WasmClient } from "../wasm-bridge";
import type { ChatSnapshot, ContextPill, AgentRun, ChatSessionSummary } from "./chat-zone";

export async function createChatSession(
  client: WasmClient | null,
  sessions: ChatSessionSummary[],
  onStatus: ((message: string) => void) | undefined,
  setBusy: (value: boolean) => void,
): Promise<string | null> {
  if (!client) return null;
  setBusy(true);
  try {
    const response = await client.dispatchChatCreate({
      title: sessions.length === 0 ? "main" : `chat ${sessions.length + 1}`,
      goal: null,
      related_files: [],
    });
    await client.dispatchChatList({ include_archived: false });
    const created = unwrapPayload(response) as { session_id?: string };
    return created.session_id ?? null;
  } catch (err) {
    reportError(onStatus)(err);
    return null;
  } finally {
    setBusy(false);
  }
}

export async function selectChatSession(
  client: WasmClient | null,
  sessionId: string,
  onStatus?: (message: string) => void,
): Promise<void> {
  if (!client) return;
  await client
    .dispatchChatHistory({ session_id: sessionId, limit: 100 })
    .catch(reportError(onStatus));
}

export async function cancelAgentRun(
  client: WasmClient | null,
  agentRun: AgentRun | null,
  onStatus?: (message: string) => void,
): Promise<void> {
  if (!client || !agentRun) return;
  await client
    .dispatchAgentCancel({ run_id: agentRun.run_id })
    .catch(reportError(onStatus));
}

export async function sendChatMessage(params: {
  client: WasmClient | null;
  draft: string;
  operation: AgentOperationLevel;
  currentSessionId: string | null;
  sessions: ChatSessionSummary[];
  agentRun: AgentRun | null;
  busy: boolean;
  contextPills: ContextPill[];
  onStatus?: (message: string) => void;
  setBusy: (value: boolean) => void;
  setDraft: (value: string) => void;
}): Promise<void> {
  if (!params.client || params.busy || params.agentRun) return;
  const content = params.draft.trim();
  if (!content) return;
  params.setBusy(true);
  try {
    await sendChatMessageInner(params, content);
  } catch (err) {
    reportError(params.onStatus)(err);
  } finally {
    params.setBusy(false);
  }
}

async function sendChatMessageInner(
  params: {
    client: WasmClient | null;
    operation: AgentOperationLevel;
    currentSessionId: string | null;
    sessions: ChatSessionSummary[];
    contextPills: ContextPill[];
    onStatus?: (message: string) => void;
    setBusy: (value: boolean) => void;
    setDraft: (value: string) => void;
  },
  content: string,
): Promise<void> {
  const client = params.client;
  if (!client) return;
  const sessionId =
    params.currentSessionId ??
    (await createChatSession(
      client,
      params.sessions,
      params.onStatus,
      params.setBusy,
    ));
  if (!sessionId) return;
  const explicitCommand = parseExplicitSlashCommand(content);
  const { operation, prompt } =
    explicitCommand ?? { operation: params.operation, prompt: content.trim() };
  const displayContent = prompt || content;
  await client.dispatchChatSend({
    session_id: sessionId,
    content: displayContent,
    related_files: [],
  });
  params.setDraft("");
  const context_refs = params.contextPills.map((pill) => pill.ref_text);
  await client.dispatchAgentInvoke({
    session_id: sessionId,
    prompt: displayContent,
    operation,
    confirmed_cadquery: null,
    context_refs,
  });
  await client.dispatchChatHistory({ session_id: sessionId, limit: 100 });
}

export async function previewPlan(
  client: WasmClient | null,
  plan: AgentPlanProposedEvent,
  onStatus?: (message: string) => void,
): Promise<void> {
  if (!client) return;
  await client
    .dispatchCadQueryPreview({
      target_path: plan.target_path,
      export_formats: [],
      params_json: "{}",
    })
    .catch(reportError(onStatus));
}

// plan.run_id identifies the plan-proposing run; the backend creates a new Execute run on confirm.
export async function confirmPlan(
  client: WasmClient | null,
  plan: AgentPlanProposedEvent,
  _snapshot: ChatSnapshot,
  onStatus?: (message: string) => void,
): Promise<void> {
  if (!client) return;
  const confirmation = {
    request: {
      target_path: plan.target_path,
      target_type: plan.target_type,
      code: "",
      export_formats: exportFormatsForTargets(plan.export_targets),
      params_json: "{}",
    },
    plan_ref: plan.plan_ref,
    affected_files: plan.affected_files,
    new_files: plan.new_files ?? [],
    export_targets: plan.export_targets,
  };
  await client
    .dispatchAgentPlanConfirm({
      session_id: plan.session_id,
      run_id: plan.run_id,
      confirmed_cadquery: confirmation,
    })
    .catch(reportError(onStatus));
}

function exportFormatsForTargets(
  targets: AgentPlanProposedEvent["export_targets"],
): CadQueryExportFormat[] {
  const formats = targets
    .map((target) => target.path_segments.at(-1) ?? "")
    .map(exportFormatFromFilename)
    .filter((format): format is CadQueryExportFormat => Boolean(format));
  return Array.from(new Set(formats));
}

function exportFormatFromFilename(
  filename: string,
): CadQueryExportFormat | null {
  const lower = filename.toLowerCase();
  if (lower.endsWith(".step")) return "step";
  if (lower.endsWith(".stl")) return "stl";
  if (lower.endsWith(".3mf")) return "three_mf";
  return null;
}

export async function rejectPlan(
  client: WasmClient | null,
  sessionId: string,
  runId: string,
  onStatus?: (message: string) => void,
): Promise<void> {
  if (!client) return;
  await client
    .dispatchAgentPlanReject({ session_id: sessionId, run_id: runId })
    .catch(reportError(onStatus));
}

type SlashCommandResult = {
  operation: AgentOperationLevel;
  prompt: string;
};

const SLASH_COMMANDS: Record<string, AgentOperationLevel> = {
  "/plan": "plan",
  "/execute": "execute",
  "/inform": "inform",
};

export function parseSlashCommand(input: string): SlashCommandResult {
  return (
    parseExplicitSlashCommand(input) ?? {
      operation: "auto",
      prompt: input.trim(),
    }
  );
}

function parseExplicitSlashCommand(input: string): SlashCommandResult | null {
  const trimmed = input.trimStart();
  for (const [prefix, operation] of Object.entries(SLASH_COMMANDS)) {
    if (!trimmed.startsWith(prefix)) continue;
    const afterCmd = trimmed.slice(prefix.length);
    if (afterCmd.length === 0 || /^\s/.test(afterCmd)) {
      return { operation, prompt: afterCmd.trim() };
    }
  }
  return null;
}

function unwrapPayload(response: unknown): unknown {
  if (!response || typeof response !== "object") return response;
  const record = response as Record<string, unknown>;
  return record["payload"] ?? response;
}

export function reportError(onStatus?: (message: string) => void) {
  return (err: unknown) => {
    onStatus?.(err instanceof Error ? err.message : String(err));
  };
}

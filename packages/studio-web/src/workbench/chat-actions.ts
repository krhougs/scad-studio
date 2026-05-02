import type { AgentMode } from "@budn/app-server-protocol";
import type { WasmClient } from "../wasm-bridge";
import type {
  AgentModelSelection,
  ContextPill,
  AgentRun,
  ChatSessionSummary,
} from "./chat-zone";

export async function createChatSession(
  client: WasmClient | null,
  sessions: ChatSessionSummary[],
  onStatus: ((message: string) => void) | undefined,
  clientRequestId?: string | null,
  initialUserMessage?: string | null,
): Promise<string | null> {
  if (!client) return null;
  try {
    const backendSessionCount = sessions.filter(
      (session) => !session.client_request_id,
    ).length;
    const response = await client.dispatchChatCreate({
      title: backendSessionCount === 0 ? "main" : `chat ${backendSessionCount + 1}`,
      goal: null,
      related_files: [],
      client_request_id: clientRequestId ?? null,
      initial_user_message: initialUserMessage ?? null,
    });
    const created = unwrapPayload(response) as { session_id?: string };
    await client
      .dispatchChatList({ include_archived: false })
      .catch(reportError(onStatus));
    return created.session_id ?? null;
  } catch (err) {
    reportError(onStatus)(err);
    return null;
  }
}

export async function selectChatSession(
  client: WasmClient | null,
  sessionId: string,
  onStatus?: (message: string) => void,
): Promise<void> {
  if (!client) return;
  await client
    .dispatchChatSelect(sessionId, { session_id: sessionId, limit: 100 })
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
  mode: AgentMode;
  currentSessionId: string | null;
  sessions: ChatSessionSummary[];
  agentRun: AgentRun | null;
  busy: boolean;
  contextPills: ContextPill[];
  agentModelSelection?: AgentModelSelection | null;
  draftClientRequestId?: string | null;
  onStatus?: (message: string) => void;
  setBusy: (value: boolean) => void;
}, text: string): Promise<boolean> {
  if (!params.client || params.busy || params.agentRun) return false;
  const content = text.trim();
  if (!content) return false;
  params.setBusy(true);
  try {
    return await sendChatMessageInner(params, content);
  } catch (err) {
    reportError(params.onStatus)(err);
    return false;
  } finally {
    params.setBusy(false);
  }
}

export async function runSavedPlan(params: {
  client: WasmClient | null;
  planId: string;
  planRef: unknown;
  currentSessionId: string | null;
  sessions: ChatSessionSummary[];
  agentRun: AgentRun | null;
  busy: boolean;
  contextPills: ContextPill[];
  agentModelSelection?: AgentModelSelection | null;
  draftClientRequestId?: string | null;
  onStatus?: (message: string) => void;
  setBusy: (value: boolean) => void;
}): Promise<boolean> {
  if (!params.client || params.busy || params.agentRun) return false;
  params.setBusy(true);
  try {
    return await runSavedPlanInner(params);
  } catch (err) {
    reportError(params.onStatus)(err);
    return false;
  } finally {
    params.setBusy(false);
  }
}

async function sendChatMessageInner(
  params: {
    client: WasmClient | null;
    mode: AgentMode;
    currentSessionId: string | null;
    sessions: ChatSessionSummary[];
    contextPills: ContextPill[];
    agentModelSelection?: AgentModelSelection | null;
    draftClientRequestId?: string | null;
    onStatus?: (message: string) => void;
    setBusy: (value: boolean) => void;
  },
  content: string,
): Promise<boolean> {
  const client = params.client;
  if (!client) return false;
  const explicitCommand = parseExplicitSlashCommand(content);
  const { mode, prompt } =
    explicitCommand ?? { mode: params.mode, prompt: content.trim() };
  const displayContent = prompt || content;
  const clientRequestId =
    params.draftClientRequestId ??
    (params.currentSessionId ? null : newClientRequestId());
  const sessionId =
    params.currentSessionId ??
    (await createChatSession(
      client,
      params.sessions,
      params.onStatus,
      clientRequestId,
      displayContent,
    ));
  if (!sessionId) return false;
  const createdNewSession = !params.currentSessionId;
  if (params.currentSessionId) {
    await client.dispatchChatSend({
      session_id: sessionId,
      content: displayContent,
      related_files: [],
      client_request_id: clientRequestId,
    });
  }
  const context_refs = params.contextPills.map((pill) => pill.ref_text);
  try {
    await client.dispatchAgentInvoke({
      session_id: sessionId,
      client_request_id: clientRequestId,
      prompt: displayContent,
      mode,
      plan_ref: null,
      context_refs,
      ...agentModelInvokeFields(params.agentModelSelection ?? null),
    });
  } catch (err) {
    if (!createdNewSession) throw err;
    reportError(params.onStatus)(err);
    await client.dispatchChatHistory({ session_id: sessionId, limit: 100 }).catch(() => undefined);
    return true;
  }
  await client.dispatchChatHistory({ session_id: sessionId, limit: 100 });
  return true;
}

async function runSavedPlanInner(params: {
  client: WasmClient | null;
  planId: string;
  planRef: unknown;
  currentSessionId: string | null;
  sessions: ChatSessionSummary[];
  contextPills: ContextPill[];
  agentModelSelection?: AgentModelSelection | null;
  draftClientRequestId?: string | null;
  onStatus?: (message: string) => void;
  setBusy: (value: boolean) => void;
}): Promise<boolean> {
  const client = params.client;
  if (!client) return false;
  const prompt = `Run plan ${params.planId}`;
  const clientRequestId =
    params.draftClientRequestId ??
    (params.currentSessionId ? null : newClientRequestId());
  const sessionId =
    params.currentSessionId ??
    (await createChatSession(
      client,
      params.sessions,
      params.onStatus,
      clientRequestId,
      prompt,
    ));
  if (!sessionId) return false;
  const createdNewSession = !params.currentSessionId;
  params.onStatus?.(`Running plan ${params.planId} in Agent mode`);
  try {
    await client.dispatchAgentInvoke({
      session_id: sessionId,
      client_request_id: clientRequestId,
      prompt,
      mode: "agent",
      plan_ref: params.planRef,
      context_refs: params.contextPills.map((pill) => pill.ref_text),
      ...agentModelInvokeFields(params.agentModelSelection ?? null),
    });
  } catch (err) {
    if (!createdNewSession) throw err;
    reportError(params.onStatus)(err);
    await client.dispatchChatHistory({ session_id: sessionId, limit: 100 }).catch(() => undefined);
    return true;
  }
  await client.dispatchChatHistory({ session_id: sessionId, limit: 100 });
  return true;
}

type SlashCommandResult = {
  mode: AgentMode;
  prompt: string;
};

const SLASH_COMMANDS: Record<string, AgentMode> = {
  "/agent": "agent",
  "/plan": "plan",
};

export function parseSlashCommand(input: string): SlashCommandResult {
  return (
    parseExplicitSlashCommand(input) ?? {
      mode: "agent",
      prompt: input.trim(),
    }
  );
}

function parseExplicitSlashCommand(input: string): SlashCommandResult | null {
  const trimmed = input.trimStart();
  for (const [prefix, mode] of Object.entries(SLASH_COMMANDS)) {
    if (!trimmed.startsWith(prefix)) continue;
    const afterCmd = trimmed.slice(prefix.length);
    if (afterCmd.length === 0 || /^\s/.test(afterCmd)) {
      return { mode, prompt: afterCmd.trim() };
    }
  }
  return null;
}

function newClientRequestId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `request-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function unwrapPayload(response: unknown): unknown {
  if (!response || typeof response !== "object") return response;
  const record = response as Record<string, unknown>;
  return record["payload"] ?? response;
}

function agentModelInvokeFields(selection: AgentModelSelection | null) {
  return {
    provider_id: selection?.provider_id ?? null,
    model_id: selection?.model_id ?? null,
    reasoning_effort: selection?.reasoning_effort ?? null,
    service_label: selection?.service_label ?? null,
  };
}

export function reportError(onStatus?: (message: string) => void) {
  return (err: unknown) => {
    onStatus?.(err instanceof Error ? err.message : String(err));
  };
}

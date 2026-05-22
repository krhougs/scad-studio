import type { AgentMode, AgentProviderType } from "@budn/app-server-protocol";
import type { WasmClient } from "../wasm-bridge";
import type {
  AgentModelSelection,
  AgentModelRegistry,
  AgentModelRegistryModel,
  AgentModelRegistryProvider,
  AgentRun,
  ChatSessionSummary,
} from "./chat-zone";

export async function createChatSession(
  client: WasmClient | null,
  sessions: ChatSessionSummary[],
  onStatus: ((message: string) => void) | undefined,
  clientRequestId?: string | null,
  initialUserMessage?: string | null,
  agentModelSelection?: AgentModelSelection | null,
  initialTurn?: {
    mode: AgentMode;
    plan_ref: unknown | null;
  } | null,
): Promise<{ sessionId: string; agentRun: AgentRun | null } | null> {
  if (!client) return null;
  if (initialTurn && !agentModelSelection) {
    onStatus?.("当前没有可用的 Agent 模型，无法创建 chat");
    return null;
  }
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
      requested_model: boundAgentModel(agentModelSelection ?? null),
      initial_turn: initialTurn ?? null,
    });
    const created = unwrapPayload(response) as {
      session_id?: string;
      initial_turn?: AgentRun | null;
    };
    await client
      .dispatchChatList({ include_archived: false })
      .catch(reportError(onStatus));
    return created.session_id
      ? { sessionId: created.session_id, agentRun: created.initial_turn ?? null }
      : null;
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
  const agentId = agentRun.agent_id;
  if (!agentId) return;
  await client
    .dispatchAgentCancel({ agent_id: agentId })
    .catch(reportError(onStatus));
}

export async function sendChatMessage(params: {
  client: WasmClient | null;
  mode: AgentMode;
  currentSessionId: string | null;
  sessions: ChatSessionSummary[];
  agentRun: AgentRun | null;
  busy: boolean;
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
      params.agentModelSelection ?? null,
      {
        mode,
        plan_ref: null,
      },
    ))?.sessionId;
  if (!sessionId) return false;
  const createdNewSession = !params.currentSessionId;
  const agentId = createdNewSession
    ? null
    : agentIdForSession(params.sessions, sessionId);
  if (!createdNewSession && !agentId) {
    params.onStatus?.("当前 chat 缺少 agent_id，无法启动 Agent turn");
    return false;
  }
  if (params.currentSessionId) {
    await client.dispatchChatSend({
      session_id: sessionId,
      content: displayContent,
      related_files: [],
      client_request_id: clientRequestId,
    });
  }
  if (createdNewSession) {
    await client.dispatchChatHistory({ session_id: sessionId, limit: 100 });
    return true;
  }
  try {
    await client.dispatchAgentStartTurn({
      agent_id: agentId,
      client_request_id: clientRequestId,
      prompt: displayContent,
      mode,
      plan_ref: null,
    });
  } catch (err) {
    throw err;
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
      params.agentModelSelection ?? null,
      {
        mode: "agent",
        plan_ref: params.planRef,
      },
    ))?.sessionId;
  if (!sessionId) return false;
  const createdNewSession = !params.currentSessionId;
  params.onStatus?.(`Running plan ${params.planId} in Agent mode`);
  if (createdNewSession) {
    await client.dispatchChatHistory({ session_id: sessionId, limit: 100 });
    return true;
  }
  const agentId = agentIdForSession(params.sessions, sessionId);
  if (!agentId) {
    params.onStatus?.("当前 chat 缺少 agent_id，无法启动 Agent turn");
    return false;
  }
  try {
    await client.dispatchAgentStartTurn({
      agent_id: agentId,
      client_request_id: clientRequestId,
      prompt,
      mode: "agent",
      plan_ref: params.planRef,
    });
  } catch (err) {
    throw err;
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

type ActiveAgentModel = {
  provider: AgentModelRegistryProvider;
  model: AgentModelRegistryModel;
};

export function activeAgentModelSelection(
  registry: AgentModelRegistry | null,
): AgentModelSelection | null {
  const active = activeAgentModel(registry);
  if (!active || !registry) return null;
  return {
    provider_id: registry.active_provider_id,
    provider_type: active.provider.kind as AgentProviderType,
    model_id: registry.active_model_id,
    reasoning_effort: registry.active_reasoning_effort,
    service_label: registry.active_service_label,
  };
}

function activeAgentModel(registry: AgentModelRegistry | null): ActiveAgentModel | null {
  if (!registry) return null;
  for (const provider of registry.providers) {
    if (provider.id !== registry.active_provider_id) continue;
    const model = provider.models.find((item) => item.id === registry.active_model_id);
    return model ? { provider, model } : null;
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

function boundAgentModel(selection: AgentModelSelection | null) {
  if (!selection) return null;
  return {
    provider_id: selection.provider_id,
    provider_type: selection.provider_type,
    model_id: selection.model_id,
    reasoning_effort: selection.reasoning_effort,
    service_label: selection.service_label,
  };
}

function agentIdForSession(sessions: ChatSessionSummary[], sessionId: string): string | null {
  return sessions.find((session) => session.session_id === sessionId)?.agent_id ?? null;
}

export function reportError(onStatus?: (message: string) => void) {
  return (err: unknown) => {
    onStatus?.(err instanceof Error ? err.message : String(err));
  };
}

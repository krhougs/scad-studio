import { create } from "zustand";
import { useShallow } from "zustand/react/shallow";
import type {
  ChatSessionSummary,
  ChatMessageRecord,
  AgentRun,
  AgentEvent,
  ChatSnapshot,
  AgentProviderCapabilities,
  AgentModelRegistry,
  AgentModelRegistryModel,
  AgentModelRegistryProvider,
} from "../workbench/chat-zone";
import type {
  CadQueryResultReady,
  AgentRuntimeStatus,
  SelectionUpdateRequest,
} from "@budn/app-server-protocol";

export type WorkspaceSlice = {
  workspace_current: { workspace_id?: unknown; root_name?: string } | null;
};

export type ChatSlice = {
  chat_sessions: ChatSessionSummary[];
  current_chat_session: string | null;
  current_chat_history: ChatMessageRecord[];
  agent_run: AgentRun | null;
  agent_runtime_status: AgentRuntimeStatus | null;
  agent_events: AgentEvent[];
  agent_event_records: AgentEventRecord[];
  current_selection: SelectionUpdateRequest | null;
  cadquery_results: CadQueryResultReady[];
  llm_configured: boolean;
  agent_provider: AgentProviderCapabilities | null;
  agent_model_registry: AgentModelRegistry | null;
};

export type TransportSlice = {
  transport_status: string | null;
};

export type ProtocolState = WorkspaceSlice & ChatSlice & TransportSlice;

export type ProtocolStore = ProtocolState & {
  applySnapshot: (raw: unknown) => void;
};

const INITIAL_WORKSPACE: WorkspaceSlice = {
  workspace_current: null,
};

const INITIAL_CHAT: ChatSlice = {
  chat_sessions: [],
  current_chat_session: null,
  current_chat_history: [],
  agent_run: null,
  agent_runtime_status: null,
  agent_events: [],
  agent_event_records: [],
  current_selection: null,
  cadquery_results: [],
  llm_configured: true,
  agent_provider: null,
  agent_model_registry: null,
};

const INITIAL_TRANSPORT: TransportSlice = {
  transport_status: null,
};

export const useProtocolStore = create<ProtocolStore>((set, get) => ({
  ...INITIAL_WORKSPACE,
  ...INITIAL_CHAT,
  ...INITIAL_TRANSPORT,
  applySnapshot: (raw: unknown) => {
    if (!raw || typeof raw !== "object") return;
    const snap = raw as Record<string, unknown>;
    const state = get();
    const patch: Partial<ProtocolState> = {};

    applyWorkspaceFields(snap, state, patch);
    applyChatFields(snap, state, patch);
    applyTransportFields(snap, state, patch);

    if (Object.keys(patch).length > 0) {
      set(patch);
    }
  },
}));

export function useChatSnapshot(): ChatSnapshot {
  return useProtocolStore(useShallow(chatSnapshotSelector));
}

export function useWorkspaceName(): string {
  return useProtocolStore(workspaceNameSelector);
}

export function useAgentRun(): AgentRun | null {
  return useProtocolStore(agentRunSelector);
}

export function useChatSessions(): ChatSessionSummary[] {
  return useProtocolStore(chatSessionsSelector);
}

export function useCurrentChatSession(): string | null {
  return useProtocolStore(currentChatSessionSelector);
}

export function useTransportStatus(): string | null {
  return useProtocolStore(transportStatusSelector);
}

function chatSnapshotSelector(s: ProtocolState): ChatSnapshot {
  return {
    workspace_current: s.workspace_current,
    chat_sessions: s.chat_sessions,
    current_chat_session: s.current_chat_session,
    current_chat_history: s.current_chat_history,
    agent_run: s.agent_run,
    agent_runtime_status: s.agent_runtime_status,
    agent_events: s.agent_events,
    current_selection: s.current_selection,
    llm_configured: s.llm_configured,
    agent_provider: s.agent_provider,
    agent_model_registry: s.agent_model_registry,
  };
}

function workspaceNameSelector(s: ProtocolState): string {
  return s.workspace_current?.root_name ?? "(loading)";
}

function agentRunSelector(s: ProtocolState): AgentRun | null {
  return s.agent_run;
}

function chatSessionsSelector(s: ProtocolState): ChatSessionSummary[] {
  return s.chat_sessions;
}

function currentChatSessionSelector(s: ProtocolState): string | null {
  return s.current_chat_session;
}

function transportStatusSelector(s: ProtocolState): string | null {
  return s.transport_status;
}

// --- structural comparison helpers ---

function applyWorkspaceFields(
  snap: Record<string, unknown>,
  state: ProtocolState,
  patch: Partial<ProtocolState>,
): void {
  const next = (snap["workspace_current"] as WorkspaceSlice["workspace_current"]) ?? null;
  if (!workspaceCurrentEqual(state.workspace_current, next)) {
    patch.workspace_current = next;
  }
}

function applyChatFields(
  snap: Record<string, unknown>,
  state: ProtocolState,
  patch: Partial<ProtocolState>,
): void {
  const sessions = (snap["chat_sessions"] as ChatSessionSummary[] | undefined) ?? [];
  if (!chatSessionsEqual(state.chat_sessions, sessions)) {
    patch.chat_sessions = sessions;
  }

  const currentSession = (snap["current_chat_session"] as string | undefined) ?? null;
  if (state.current_chat_session !== currentSession) {
    patch.current_chat_session = currentSession;
  }

  const history = (snap["current_chat_history"] as ChatMessageRecord[] | undefined) ?? [];
  if (!chatHistoryEqual(state.current_chat_history, history)) {
    patch.current_chat_history = history;
  }

  const agentRun = (snap["agent_run"] as AgentRun | undefined) ?? null;
  if (!agentRunEqual(state.agent_run, agentRun)) {
    patch.agent_run = agentRun;
  }
  const agentRuntimeStatus =
    (snap["agent_runtime_status"] as AgentRuntimeStatus | undefined) ?? null;
  if (state.agent_runtime_status !== agentRuntimeStatus) {
    patch.agent_runtime_status = agentRuntimeStatus;
  }

  const events = (snap["agent_events"] as AgentEvent[] | undefined) ?? [];
  const eventRecords =
    (snap["agent_event_records"] as AgentEventRecord[] | undefined) ?? [];
  const renderedEvents = [...agentEventsFromRecords(eventRecords), ...events];
  if (!agentEventsEqual(state.agent_events, renderedEvents)) {
    patch.agent_events = renderedEvents;
  }
  if (!agentEventRecordsEqual(state.agent_event_records, eventRecords)) {
    patch.agent_event_records = eventRecords;
  }

  const selection = (snap["current_selection"] as SelectionUpdateRequest | undefined) ?? null;
  if (state.current_selection !== selection) {
    patch.current_selection = selection;
  }

  const cadqueryResults =
    (snap["cadquery_results"] as CadQueryResultReady[] | undefined) ?? [];
  if (!cadQueryResultsEqual(state.cadquery_results, cadqueryResults)) {
    patch.cadquery_results = cadqueryResults;
  }

  const llm = (snap["llm_configured"] as boolean | undefined) ?? true;
  if (state.llm_configured !== llm) {
    patch.llm_configured = llm;
  }

  const agentProvider =
    (snap["agent_provider"] as AgentProviderCapabilities | undefined) ?? null;
  if (!agentProviderEqual(state.agent_provider, agentProvider)) {
    patch.agent_provider = agentProvider;
  }

  const modelRegistry =
    (snap["agent_model_registry"] as AgentModelRegistry | undefined) ?? null;
  if (!agentModelRegistryEqual(state.agent_model_registry, modelRegistry)) {
    patch.agent_model_registry = modelRegistry;
  }
}

function applyTransportFields(
  snap: Record<string, unknown>,
  state: ProtocolState,
  patch: Partial<ProtocolState>,
): void {
  const status = (snap["transport_status"] as string | undefined) ?? null;
  if (state.transport_status !== status) {
    patch.transport_status = status;
  }
}

function workspaceCurrentEqual(
  a: WorkspaceSlice["workspace_current"],
  b: WorkspaceSlice["workspace_current"],
): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  return a.root_name === b.root_name && a.workspace_id === b.workspace_id;
}

export function chatSessionsEqual(
  a: ChatSessionSummary[],
  b: ChatSessionSummary[],
): boolean {
  if (a === b) return true;
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    const sa = a[i]!;
    const sb = b[i]!;
    if (
      sa.session_id !== sb.session_id ||
      sa.title !== sb.title ||
      sa.archived !== sb.archived ||
      sa.message_count !== sb.message_count ||
      sa.agent_id !== sb.agent_id ||
      JSON.stringify(sa.bound_model ?? null) !== JSON.stringify(sb.bound_model ?? null) ||
      !pathHandlesEqual(sa.related_files, sb.related_files)
    ) return false;
  }
  return true;
}

function pathHandlesEqual(a: unknown[] | undefined, b: unknown[] | undefined): boolean {
  const left = a ?? [];
  const right = b ?? [];
  if (left.length !== right.length) return false;
  for (let i = 0; i < left.length; i++) {
    if (JSON.stringify(left[i]) !== JSON.stringify(right[i])) return false;
  }
  return true;
}

export function chatHistoryEqual(
  a: ChatMessageRecord[],
  b: ChatMessageRecord[],
): boolean {
  if (a === b) return true;
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    const ma = a[i]!;
    const mb = b[i]!;
    if (ma.message_id !== mb.message_id || ma.ts_ms !== mb.ts_ms) return false;
    if (!searchSourcesEqual(ma.search_sources ?? [], mb.search_sources ?? [])) {
      return false;
    }
  }
  return true;
}

function searchSourcesEqual(
  a: ChatMessageRecord["search_sources"],
  b: ChatMessageRecord["search_sources"],
): boolean {
  if (a === b) return true;
  const left = a ?? [];
  const right = b ?? [];
  if (left.length !== right.length) return false;
  for (let i = 0; i < left.length; i++) {
    const sa = left[i]!;
    const sb = right[i]!;
    if (
      sa.title !== sb.title ||
      sa.url !== sb.url ||
      sa.start_index !== sb.start_index ||
      sa.end_index !== sb.end_index
    ) return false;
  }
  return true;
}

export function agentRunEqual(a: AgentRun | null, b: AgentRun | null): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  return (
    a.session_id === b.session_id &&
    a.agent_id === b.agent_id &&
    a.run_id === b.run_id &&
    a.turn_id === b.turn_id
  );
}

export function agentEventsEqual(a: AgentEvent[], b: AgentEvent[]): boolean {
  if (a === b) return true;
  if (a.length !== b.length) return false;
  return a.every((left, index) => {
    const right = b[index]!;
    return (
      left.event === right.event &&
      JSON.stringify(left.payload ?? {}) === JSON.stringify(right.payload ?? {})
    );
  });
}

type AgentEventRecord = {
  event_id: number | { 0?: number } | Record<string, unknown>;
  agent_id: string | { 0?: string } | Record<string, unknown>;
  turn_id?: string | { 0?: string } | null | Record<string, unknown>;
  ts_ms: number;
  payload: {
    event?: string;
    payload?: Record<string, unknown>;
  };
};

function agentEventsFromRecords(records: AgentEventRecord[]): AgentEvent[] {
  return records
    .map(agentEventFromRecord)
    .filter((event): event is AgentEvent => event !== null);
}

function agentEventFromRecord(record: AgentEventRecord): AgentEvent | null {
  const payload = record.payload?.payload ?? {};
  const event = record.payload?.event;
  const turnId = idString(record.turn_id ?? null);
  const metadata = {
    agent_id: idString(record.agent_id),
    turn_id: turnId,
    run_id: turnId,
  };
  if (event === "token") {
    return { event: "agent.token", payload: { ...metadata, ...payload } };
  }
  if (event === "reasoning") {
    return { event: "agent.reasoning", payload: { ...metadata, ...payload } };
  }
  if (event === "tool_start") {
    return { event: "agent.tool_start", payload: { ...metadata, ...payload } };
  }
  if (event === "tool_result") {
    return { event: "agent.tool_result", payload: { ...metadata, ...payload } };
  }
  if (event === "hosted_tool_activity") {
    return { event: "agent.hosted_tool_activity", payload: { ...metadata, ...payload } };
  }
  if (event === "error") {
    return { event: "agent.error", payload: { ...metadata, ...payload } };
  }
  if (event === "done") {
    return { event: "agent.done", payload: { ...metadata, ...payload } };
  }
  if (event === "state_changed") {
    return { event: "agent.state_changed", payload: { ...metadata, ...payload } };
  }
  return null;
}

function idString(value: unknown): string | null {
  if (typeof value === "string") return value;
  if (!value || typeof value !== "object") return null;
  const record = value as Record<string, unknown>;
  const inner = record["0"];
  return typeof inner === "string" ? inner : null;
}

function agentEventRecordsEqual(
  a: AgentEventRecord[],
  b: AgentEventRecord[],
): boolean {
  if (a === b) return true;
  if (a.length !== b.length) return false;
  if (a.length === 0) return true;
  const left = a[a.length - 1]!;
  const right = b[b.length - 1]!;
  return (
    JSON.stringify(left.event_id) === JSON.stringify(right.event_id) &&
    JSON.stringify(left.agent_id) === JSON.stringify(right.agent_id)
  );
}

export function agentProviderEqual(
  a: AgentProviderCapabilities | null,
  b: AgentProviderCapabilities | null,
): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  return (
    a.provider === b.provider &&
    a.model === b.model &&
    a.native_web_search_enabled === b.native_web_search_enabled &&
    a.search_sources_supported === b.search_sources_supported
  );
}

export function agentModelRegistryEqual(
  a: AgentModelRegistry | null,
  b: AgentModelRegistry | null,
): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  return (
    a.active_provider_id === b.active_provider_id &&
    a.active_model_id === b.active_model_id &&
    a.active_reasoning_effort === b.active_reasoning_effort &&
    a.active_reasoning_effort_applied === b.active_reasoning_effort_applied &&
    a.active_service_label === b.active_service_label &&
    a.active_service_label_applied === b.active_service_label_applied &&
    stringArrayEqual(a.reasoning_effort_options, b.reasoning_effort_options) &&
    stringArrayEqual(a.service_label_options, b.service_label_options) &&
    registryProvidersEqual(a.providers, b.providers)
  );
}

function registryProvidersEqual(
  a: AgentModelRegistryProvider[],
  b: AgentModelRegistryProvider[],
): boolean {
  if (a === b) return true;
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (!registryProviderEqual(a[i]!, b[i]!)) return false;
  }
  return true;
}

function registryProviderEqual(
  a: AgentModelRegistryProvider,
  b: AgentModelRegistryProvider,
): boolean {
  return (
    a.id === b.id &&
    a.kind === b.kind &&
    a.label === b.label &&
    a.discovery.enabled === b.discovery.enabled &&
    a.discovery.status === b.discovery.status &&
    a.discovery.error === b.discovery.error &&
    registryModelsEqual(a.models, b.models)
  );
}

function registryModelsEqual(
  a: AgentModelRegistryModel[],
  b: AgentModelRegistryModel[],
): boolean {
  if (a === b) return true;
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (!registryModelEqual(a[i]!, b[i]!)) return false;
  }
  return true;
}

function registryModelEqual(
  a: AgentModelRegistryModel,
  b: AgentModelRegistryModel,
): boolean {
  return (
    a.id === b.id &&
    a.label === b.label &&
    a.source === b.source &&
    a.reasoning_effort === b.reasoning_effort &&
    a.service_label === b.service_label &&
    a.native_web_search_enabled === b.native_web_search_enabled &&
    a.native_web_search_applied === b.native_web_search_applied &&
    a.web_search_supported === b.web_search_supported &&
    a.web_search_unsupported_reason === b.web_search_unsupported_reason &&
    a.search_sources_supported === b.search_sources_supported
  );
}

function stringArrayEqual(a: string[], b: string[]): boolean {
  if (a === b) return true;
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

export function cadQueryResultsEqual(
  a: CadQueryResultReady[],
  b: CadQueryResultReady[],
): boolean {
  if (a === b) return true;
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    const left = a[i]!;
    const right = b[i]!;
    if (
      left.result_id !== right.result_id ||
      left.build_id !== right.build_id ||
      left.part_count !== right.part_count ||
      left.face_count !== right.face_count ||
      left.edge_count !== right.edge_count ||
      left.vertex_count !== right.vertex_count ||
      !artifactRelationEqual(left.artifact_relation, right.artifact_relation)
    ) {
      return false;
    }
  }
  return true;
}

function artifactRelationEqual(
  a: CadQueryResultReady["artifact_relation"],
  b: CadQueryResultReady["artifact_relation"],
): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  if (a.source_path !== b.source_path || a.exports.length !== b.exports.length) {
    return false;
  }
  for (let i = 0; i < a.exports.length; i++) {
    const left = a.exports[i]!;
    const right = b.exports[i]!;
    if (
      left.name !== right.name ||
      left.path !== right.path ||
      left.hash !== right.hash
    ) {
      return false;
    }
  }
  return true;
}

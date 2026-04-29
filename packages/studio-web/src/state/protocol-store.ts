import { create } from "zustand";
import { useShallow } from "zustand/react/shallow";
import type {
  ChatSessionSummary,
  ChatMessageRecord,
  AgentRun,
  AgentEvent,
  ChatSnapshot,
} from "../workbench/chat-zone";
import type { SelectionUpdateRequest } from "@budn/app-server-protocol";

export type WorkspaceSlice = {
  workspace_current: { workspace_id?: unknown; root_name?: string } | null;
};

export type ChatSlice = {
  chat_sessions: ChatSessionSummary[];
  current_chat_session: string | null;
  current_chat_history: ChatMessageRecord[];
  agent_run: AgentRun | null;
  agent_events: AgentEvent[];
  current_selection: SelectionUpdateRequest | null;
  llm_configured: boolean;
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
  agent_events: [],
  current_selection: null,
  llm_configured: true,
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
    agent_events: s.agent_events,
    current_selection: s.current_selection,
    llm_configured: s.llm_configured,
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

  const events = (snap["agent_events"] as AgentEvent[] | undefined) ?? [];
  if (!agentEventsEqual(state.agent_events, events)) {
    patch.agent_events = events;
  }

  const selection = (snap["current_selection"] as SelectionUpdateRequest | undefined) ?? null;
  if (state.current_selection !== selection) {
    patch.current_selection = selection;
  }

  const llm = (snap["llm_configured"] as boolean | undefined) ?? true;
  if (state.llm_configured !== llm) {
    patch.llm_configured = llm;
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
      sa.message_count !== sb.message_count
    ) return false;
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
  }
  return true;
}

export function agentRunEqual(a: AgentRun | null, b: AgentRun | null): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  return a.session_id === b.session_id && a.run_id === b.run_id;
}

export function agentEventsEqual(a: AgentEvent[], b: AgentEvent[]): boolean {
  if (a === b) return true;
  if (a.length !== b.length) return false;
  if (a.length === 0) return true;
  return a[a.length - 1]!.event === b[b.length - 1]!.event;
}

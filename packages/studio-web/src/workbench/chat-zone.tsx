import { memo, useEffect, useMemo, useRef, useState } from "react";
import { Stop } from "@phosphor-icons/react";
import type {
  AgentMode,
  SelectionRef,
  SelectionUpdateRequest,
} from "@budn/app-server-protocol";
import { AssistantRuntimeProvider } from "@assistant-ui/react";
import type { WasmClient } from "../wasm-bridge";
import { useChatSnapshot } from "../state/protocol-store";
import { preferredRefText } from "./cadquery-agent-scope";
import { ChatBody } from "./chat-messages";
import { ChatComposer } from "./chat-composer";
import { useChatRuntime } from "./chat-runtime";
import {
  cancelAgentRun,
  createChatSession,
  reportError,
  runSavedPlan,
  selectChatSession,
  sendChatMessage,
} from "./chat-actions";

type ChatZoneProps = {
  client: WasmClient | null;
  onStatus?: (message: string) => void;
  onOpenPlan?: (path: unknown) => void;
};

export type ChatSnapshot = {
  workspace_current?: { workspace_id?: unknown } | null;
  chat_sessions?: ChatSessionSummary[];
  current_chat_session?: string | null;
  current_chat_history?: ChatMessageRecord[];
  agent_run?: AgentRun | null;
  agent_events?: AgentEvent[];
  current_selection?: SelectionUpdateRequest | null;
  llm_configured?: boolean;
  agent_provider?: AgentProviderCapabilities | null;
};

export type ChatSessionSummary = {
  session_id: string;
  title: string;
  archived: boolean;
  message_count: number;
};

export type ChatMessageRecord = {
  message_id: string;
  ts_ms: number;
  role: "user" | "assistant" | "tool" | "meta";
  content: string;
  related_files?: unknown[];
  tool_call_id?: string | null;
  tool_calls?: unknown[];
  tool_result?: unknown | null;
  mesh_result?: unknown | null;
  search_sources?: AgentSearchSource[];
  run_id?: string | null;
};

export type AgentProviderCapabilities = {
  provider: string;
  model?: string | null;
  native_web_search_enabled: boolean;
  search_sources_supported: boolean;
};

export type AgentSearchSource = {
  title: string;
  url: string;
  start_index?: number | null;
  end_index?: number | null;
};

export type AgentRun = {
  session_id: string;
  run_id: string;
};

export type AgentEvent = {
  event: string;
  payload?: Record<string, unknown>;
};

export type ContextPill = {
  ref_text: string;
  display: string;
};

const MAX_CONTEXT_PILLS = 3;

export const ChatZone = memo(function ChatZone({ client, onStatus, onOpenPlan }: ChatZoneProps) {
  const snapshot = useChatSnapshot();
  const controller = useChatController({ client, snapshot, onStatus, onOpenPlan });
  const runtime = useChatRuntime({
    messages: controller.messages,
    agentEvents: controller.agentEvents,
    agentRun: controller.agentRun,
    onNew: controller.send,
    onCancel: controller.cancelAgent,
    disabled: controller.composerDisabled,
  });

  return (
    <section className="chat" data-testid="workbench-chat" aria-label="agent">
      <ChatHeader
        sessions={controller.sessions}
        currentSessionId={controller.currentSessionId}
        disabled={controller.headerDisabled}
        agentRun={controller.agentRun}
        llmConfigured={snapshot.llm_configured ?? true}
        agentProvider={snapshot.agent_provider ?? null}
        onNew={controller.createSession}
        onSelect={controller.selectSession}
        onCancel={controller.cancelAgent}
      />
      <AssistantRuntimeProvider runtime={runtime}>
        <ChatBody
          llmConfigured={snapshot.llm_configured ?? true}
          planActionDisabled={controller.composerDisabled}
          onOpenPlan={controller.openPlan}
          onRunPlan={controller.runPlan}
        />
        <ChatComposer
          disabled={controller.composerDisabled}
          mode={controller.mode}
          contextPills={controller.contextPills}
          onModeChange={controller.setMode}
          onRemovePill={controller.removePill}
        />
      </AssistantRuntimeProvider>
    </section>
  );
});

function useChatController({ client, snapshot, onStatus, onOpenPlan }: ChatZoneProps & { snapshot: ChatSnapshot }) {
  const [mode, setMode] = useState<AgentMode>("agent");
  const [busy, setBusy] = useState(false);
  const [removedRefs, setRemovedRefs] = useState<Set<string>>(new Set());
  const sessions = snapshot?.chat_sessions ?? [];
  const snapshotCurrentSessionId = snapshot?.current_chat_session ?? null;
  const currentSessionId =
    snapshotCurrentSessionId ?? sessions[0]?.session_id ?? null;
  const messages = snapshot?.current_chat_history ?? [];
  const agentRun = snapshot?.agent_run ?? null;
  const rawEvents = snapshot?.agent_events ?? [];
  const sessionEvents = useMemo(
    () =>
      rawEvents.filter((event) =>
        eventBelongsToCurrentSession(event, currentSessionId),
      ),
    [rawEvents, currentSessionId],
  );
  const agentEvents = useMemo(
    () => recentAgentEvents(sessionEvents, agentRun),
    [sessionEvents, agentRun],
  );

  const contextPills = useMemo(() => {
    const selections = snapshot?.current_selection?.selections ?? [];
    return buildContextPills(selections, removedRefs);
  }, [snapshot?.current_selection, removedRefs]);

  const prevSelectionRef = useRef(snapshot?.current_selection);
  useEffect(() => {
    if (prevSelectionRef.current !== snapshot?.current_selection) {
      prevSelectionRef.current = snapshot?.current_selection;
      setRemovedRefs(new Set());
    }
  }, [snapshot?.current_selection]);

  useInitialChatList(client, onStatus);
  useInitialChatHistory(
    client,
    sessions,
    snapshotCurrentSessionId,
    onStatus,
  );
  useAgentDoneHistoryRefresh(client, currentSessionId, agentEvents, onStatus);

  const actions = useChatActions({
    client,
    sessions,
    currentSessionId,
    agentRun,
    busy,
    mode,
    contextPills,
    onStatus,
    setBusy,
  });

  return {
    mode,
    setMode,
    sessions,
    currentSessionId,
    messages,
    agentRun,
    agentEvents,
    contextPills,
    headerDisabled: !client || busy,
    composerDisabled: !client || busy || Boolean(agentRun),
    removePill: (refText: string) => {
      setRemovedRefs((prev) => new Set(prev).add(refText));
    },
    openPlan: onOpenPlan,
    ...actions,
  };
}

function eventBelongsToCurrentSession(
  event: AgentEvent,
  currentSessionId: string | null,
): boolean {
  if (!currentSessionId) return true;
  const sessionId = event.payload?.["session_id"];
  if (typeof sessionId !== "string") return true;
  return sessionId === currentSessionId;
}

function buildContextPills(
  selections: SelectionRef[],
  removedRefs: Set<string>,
): ContextPill[] {
  return selections
    .filter((sel) => !removedRefs.has(sel.ref_text))
    .slice(-MAX_CONTEXT_PILLS)
    .map((sel) => ({
      ref_text: sel.ref_text,
      display: preferredRefText(sel),
    }));
}

function useInitialChatList(
  client: WasmClient | null,
  onStatus: ((message: string) => void) | undefined,
) {
  useEffect(() => {
    if (!client) return;
    client
      .dispatchChatList({ include_archived: false })
      .catch(reportError(onStatus));
  }, [client, onStatus]);
}

function useInitialChatHistory(
  client: WasmClient | null,
  sessions: ChatSessionSummary[],
  snapshotCurrentSessionId: string | null,
  onStatus: ((message: string) => void) | undefined,
) {
  const requestedRef = useRef<string | null>(null);
  const targetSessionId =
    snapshotCurrentSessionId ?? sessions[0]?.session_id ?? null;
  useEffect(() => {
    if (!client || !targetSessionId) return;
    if (requestedRef.current === targetSessionId) return;
    requestedRef.current = targetSessionId;
    client
      .dispatchChatHistory({ session_id: targetSessionId, limit: 100 })
      .catch(reportError(onStatus));
  }, [client, targetSessionId, onStatus]);
}

function useAgentDoneHistoryRefresh(
  client: WasmClient | null,
  currentSessionId: string | null,
  agentEvents: AgentEvent[],
  onStatus: ((message: string) => void) | undefined,
) {
  const refreshedDoneRef = useRef<string | null>(null);
  const doneKey = lastAgentDoneKey(agentEvents);
  useEffect(() => {
    if (!client || !currentSessionId || !doneKey) return;
    if (refreshedDoneRef.current === doneKey) return;
    refreshedDoneRef.current = doneKey;
    client
      .dispatchChatHistory({ session_id: currentSessionId, limit: 100 })
      .catch(reportError(onStatus));
  }, [client, currentSessionId, doneKey, onStatus]);
}

function useChatActions(input: {
  client: WasmClient | null;
  sessions: ChatSessionSummary[];
  currentSessionId: string | null;
  agentRun: AgentRun | null;
  busy: boolean;
  mode: AgentMode;
  contextPills: ContextPill[];
  onStatus?: (message: string) => void;
  setBusy: (value: boolean) => void;
}) {
  return {
    createSession: () =>
      void createChatSession(
        input.client,
        input.sessions,
        input.onStatus,
        input.setBusy,
      ),
    selectSession: (id: string) =>
      void selectChatSession(input.client, id, input.onStatus),
    cancelAgent: () =>
      void cancelAgentRun(input.client, input.agentRun, input.onStatus),
    send: (text: string) => void sendChatMessage(input, text),
    runPlan: (plan: { planId: string; planRef: unknown }) =>
      void runSavedPlan({
        ...input,
        planId: plan.planId,
        planRef: plan.planRef,
      }),
  };
}

function ChatHeader(props: {
  sessions: ChatSessionSummary[];
  currentSessionId: string | null;
  disabled: boolean;
  agentRun: AgentRun | null;
  llmConfigured: boolean;
  agentProvider: AgentProviderCapabilities | null;
  onNew: () => void;
  onSelect: (id: string) => void;
  onCancel: () => void;
}) {
  const active = props.sessions.find(
    (session) => session.session_id === props.currentSessionId,
  );
  return (
    <header className="chat-head">
      <div>
        <div className="title">
          budn&apos; agent{" "}
          <span
            className={props.llmConfigured ? "llm-dot llm-dot--ok" : "llm-dot llm-dot--off"}
            title={props.llmConfigured ? "AI connected" : "AI not configured"}
          />
        </div>
        <div className="sub">
          {active?.title ?? "no session"}
          {props.agentProvider?.native_web_search_enabled ? " · web search" : ""}
        </div>
      </div>
      <div className="chat-session-actions">
        <select
          aria-label="chat session"
          value={props.currentSessionId ?? ""}
          disabled={props.disabled || props.sessions.length === 0}
          onChange={(event) => props.onSelect((event.target as HTMLSelectElement).value)}
        >
          {props.sessions.map((session) => (
            <option key={session.session_id} value={session.session_id}>
              {session.title}
            </option>
          ))}
        </select>
        {props.agentRun ? (
          <button
            type="button"
            className="btn btn--ghost btn--sm"
            onClick={props.onCancel}
          >
            <Stop size={13} weight="bold" aria-hidden="true" /> stop
          </button>
        ) : (
          <button
            type="button"
            className="btn btn--ghost btn--sm"
            onClick={props.onNew}
          >
            new
          </button>
        )}
      </div>
    </header>
  );
}

function recentAgentEvents(
  events: AgentEvent[],
  agentRun: AgentRun | null,
): AgentEvent[] {
  const agentEvents = events.filter((event) => event.event.startsWith("agent."));
  const visibleRunId = agentRun?.run_id ?? latestAgentRunId(agentEvents);
  if (!visibleRunId) return agentEvents;
  return agentEvents.filter((event) => {
    const runId = agentEventRunId(event);
    return !runId || runId === visibleRunId;
  });
}

function latestAgentRunId(events: AgentEvent[]): string | null {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const runId = agentEventRunId(events[index] as AgentEvent | undefined);
    if (runId) return runId;
  }
  return null;
}

function agentEventRunId(event: AgentEvent | undefined): string | null {
  const runId = event?.payload?.["run_id"];
  return typeof runId === "string" ? runId : null;
}

function lastAgentDoneKey(events: AgentEvent[]): string | null {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const event = events[index] as AgentEvent | undefined;
    if (!event || event.event !== "agent.done") continue;
    const payload = event.payload ?? {};
    const runId = typeof payload["run_id"] === "string" ? payload["run_id"] : null;
    return runId || `done-${index}`;
  }
  return null;
}

import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowUp, Cube, Paperclip, Ruler, Stop } from "@phosphor-icons/react";
import type { SelectionUpdateRequest } from "@budn/app-server-protocol";
import type { WasmClient } from "../wasm-bridge";
import {
  DEFAULT_CADQUERY_TARGET_PATH,
  buildCadQueryConfirmation,
  cadQuerySelectionSummary,
} from "./cadquery-agent-scope";

type ChatZoneProps = {
  client: WasmClient | null;
  snapshot: ChatSnapshot | null;
  onStatus?: (message: string) => void;
};

export type ChatSnapshot = {
  workspace_current?: { workspace_id?: unknown } | null;
  chat_sessions?: ChatSessionSummary[];
  current_chat_session?: string | null;
  current_chat_history?: ChatMessageRecord[];
  agent_run?: AgentRun | null;
  agent_events?: AgentEvent[];
  current_selection?: SelectionUpdateRequest | null;
};

type ChatSessionSummary = {
  session_id: string;
  title: string;
  archived: boolean;
  message_count: number;
};

type ChatMessageRecord = {
  message_id: string;
  ts_ms: number;
  role: "user" | "assistant" | "tool" | "meta";
  content: string;
  tool_calls?: unknown[];
  tool_result?: unknown | null;
  mesh_result?: unknown | null;
};

type AgentRun = {
  session_id: string;
  run_id: string;
};

type AgentEvent = {
  event: string;
  payload?: Record<string, unknown>;
};

type AgentOperation = "inform" | "plan" | "execute";

export function ChatZone({ client, snapshot, onStatus }: ChatZoneProps) {
  const controller = useChatController({ client, snapshot, onStatus });
  return (
    <section className="chat" data-testid="workbench-chat" aria-label="agent">
      <ChatHeader
        sessions={controller.sessions}
        currentSessionId={controller.currentSessionId}
        disabled={controller.headerDisabled}
        agentRun={controller.agentRun}
        onNew={controller.createSession}
        onSelect={controller.selectSession}
        onCancel={controller.cancelAgent}
      />
      <ChatBody
        messages={controller.messages}
        agentEvents={controller.agentEvents}
      />
      <ChatComposer
        value={controller.draft}
        disabled={controller.composerDisabled}
        operation={controller.operation}
        targetPath={controller.targetPath}
        selectionSummary={controller.selectionSummary}
        onChange={controller.setDraft}
        onOperationChange={controller.setOperation}
        onTargetPathChange={controller.setTargetPath}
        onSend={controller.send}
      />
    </section>
  );
}

function useChatController({ client, snapshot, onStatus }: ChatZoneProps) {
  const [draft, setDraft] = useState("");
  const [operation, setOperation] = useState<AgentOperation>("inform");
  const [targetPath, setTargetPath] = useState(DEFAULT_CADQUERY_TARGET_PATH);
  const [busy, setBusy] = useState(false);
  const sessions = snapshot?.chat_sessions ?? [];
  const currentSessionId =
    snapshot?.current_chat_session ?? sessions[0]?.session_id ?? null;
  const messages = snapshot?.current_chat_history ?? [];
  const agentRun = snapshot?.agent_run ?? null;
  const agentEvents = useMemo(
    () => recentAgentEvents(snapshot?.agent_events ?? []),
    [snapshot?.agent_events],
  );
  const selectionSummary = useMemo(
    () => cadQuerySelectionSummary(snapshot),
    [snapshot?.current_selection],
  );

  useInitialChatList(client, onStatus);
  useAgentDoneHistoryRefresh(client, currentSessionId, agentEvents, onStatus);
  const actions = useChatActions({
    client,
    sessions,
    currentSessionId,
    agentRun,
    busy,
    draft,
    operation,
    targetPath,
    snapshot,
    onStatus,
    setBusy,
    setDraft,
  });

  return {
    draft,
    setDraft,
    operation,
    setOperation,
    targetPath,
    setTargetPath,
    sessions,
    currentSessionId,
    messages,
    agentRun,
    agentEvents,
    selectionSummary,
    headerDisabled: !client || busy,
    composerDisabled: !client || busy || Boolean(agentRun),
    ...actions,
  };
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
  draft: string;
  operation: AgentOperation;
  targetPath: string;
  snapshot: ChatSnapshot | null;
  onStatus?: (message: string) => void;
  setBusy: (value: boolean) => void;
  setDraft: (value: string) => void;
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
    send: () => void sendChatMessage(input),
  };
}

function ChatBody(props: {
  messages: ChatMessageRecord[];
  agentEvents: AgentEvent[];
}) {
  return (
    <div className="chat-body" data-testid="chat-body">
      {props.messages.length === 0 ? (
        <p className="chat-empty">No active chat.</p>
      ) : (
        props.messages.map((message) => (
          <ChatMessage key={message.message_id} message={message} />
        ))
      )}
      {props.agentEvents.map((event, index) => (
        <AgentEventRow key={`${event.event}-${index}`} event={event} />
      ))}
    </div>
  );
}

function ChatHeader(props: {
  sessions: ChatSessionSummary[];
  currentSessionId: string | null;
  disabled: boolean;
  agentRun: AgentRun | null;
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
        <div className="title">budn&apos; agent</div>
        <div className="sub">{active?.title ?? "no session"}</div>
      </div>
      <div className="chat-session-actions">
        <select
          aria-label="chat session"
          value={props.currentSessionId ?? ""}
          disabled={props.disabled || props.sessions.length === 0}
          onChange={(event) => props.onSelect(event.target.value)}
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

function ChatMessage({ message }: { message: ChatMessageRecord }) {
  if (message.role === "meta") return null;
  const role = message.role === "user" ? "user" : "agent";
  return (
    <article className={`msg ${role}`}>
      <div className="who">
        <b>{message.role}</b>
        <time>{formatTime(message.ts_ms)}</time>
      </div>
      <div className="bubble">{message.content}</div>
    </article>
  );
}

function AgentEventRow({ event }: { event: AgentEvent }) {
  const label = event.event.replace("agent.", "");
  const detail = agentEventDetail(event);
  return (
    <div className="agent-op">
      <div className="op-head">
        <span className={event.event === "agent.done" ? "ok" : undefined}>
          {label}
        </span>
      </div>
      <div className="op-detail">{detail}</div>
    </div>
  );
}

function ChatComposer(props: {
  value: string;
  disabled: boolean;
  operation: AgentOperation;
  targetPath: string;
  selectionSummary: string | null;
  onChange: (value: string) => void;
  onOperationChange: (value: AgentOperation) => void;
  onTargetPathChange: (value: string) => void;
  onSend: () => void;
}) {
  return (
    <footer className="chat-input">
      <div className="wrap">
        <ChatTextarea
          value={props.value}
          onChange={props.onChange}
          onSend={props.onSend}
        />
        <ChatSelectionContext selectionSummary={props.selectionSummary} />
        <ChatComposerTools
          disabled={props.disabled}
          operation={props.operation}
          onOperationChange={props.onOperationChange}
          onSend={props.onSend}
        />
        <ExecuteTargetInput
          operation={props.operation}
          targetPath={props.targetPath}
          onTargetPathChange={props.onTargetPathChange}
        />
      </div>
    </footer>
  );
}

function ChatSelectionContext(props: { selectionSummary: string | null }) {
  if (!props.selectionSummary) return null;
  return (
    <div className="chat-selection-context" data-testid="chat-selection-context">
      <span>ref</span>
      <code>{props.selectionSummary}</code>
    </div>
  );
}

function ChatTextarea(props: {
  value: string;
  onChange: (value: string) => void;
  onSend: () => void;
}) {
  return (
    <textarea
      placeholder="Describe a change, add a feature, or ask for alternatives..."
      value={props.value}
      onChange={(ev) => props.onChange(ev.target.value)}
      onKeyDown={(ev) => {
        if (ev.key === "Enter" && (ev.metaKey || ev.ctrlKey)) props.onSend();
      }}
      data-testid="chat-input"
    />
  );
}

function ChatComposerTools(props: {
  disabled: boolean;
  operation: AgentOperation;
  onOperationChange: (value: AgentOperation) => void;
  onSend: () => void;
}) {
  return (
    <div className="tools">
      <div className="tools-left">
        <OperationSelector
          operation={props.operation}
          onOperationChange={props.onOperationChange}
        />
        <DisabledToolButtons />
      </div>
      <button
        type="button"
        className="send"
        disabled={props.disabled}
        onClick={props.onSend}
      >
        send <ArrowUp size={12} weight="bold" aria-hidden="true" />
      </button>
    </div>
  );
}

function OperationSelector(props: {
  operation: AgentOperation;
  onOperationChange: (value: AgentOperation) => void;
}) {
  return (
    <>
      {(["inform", "plan", "execute"] as const).map((operation) => (
        <OperationButton
          key={operation}
          active={props.operation === operation}
          label={operationLabel(operation)}
          onClick={() => props.onOperationChange(operation)}
        />
      ))}
    </>
  );
}

function DisabledToolButtons() {
  return (
    <>
      <button type="button" title="attach sketch" disabled>
        <Paperclip size={14} weight="bold" aria-hidden="true" />
      </button>
      <button type="button" title="reference part" disabled>
        <Cube size={14} weight="bold" aria-hidden="true" />
      </button>
      <button type="button" title="dimension pick" disabled>
        <Ruler size={14} weight="bold" aria-hidden="true" />
      </button>
    </>
  );
}

function ExecuteTargetInput(props: {
  operation: AgentOperation;
  targetPath: string;
  onTargetPathChange: (value: string) => void;
}) {
  if (props.operation !== "execute") return null;
  return (
    <input
      aria-label="cadquery target"
      className="chat-target-path"
      value={props.targetPath}
      onChange={(event) => props.onTargetPathChange(event.target.value)}
    />
  );
}

function OperationButton(props: {
  active: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className={props.active ? "mode is-active" : "mode"}
      onClick={props.onClick}
    >
      {props.label}
    </button>
  );
}

function operationLabel(operation: AgentOperation): string {
  return operation[0].toUpperCase() + operation.slice(1);
}

async function createChatSession(
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

async function selectChatSession(
  client: WasmClient | null,
  sessionId: string,
  onStatus?: (message: string) => void,
): Promise<void> {
  if (!client) return;
  await client
    .dispatchChatHistory({ session_id: sessionId, limit: 100 })
    .catch(reportError(onStatus));
}

async function cancelAgentRun(
  client: WasmClient | null,
  agentRun: AgentRun | null,
  onStatus?: (message: string) => void,
): Promise<void> {
  if (!client || !agentRun) return;
  await client
    .dispatchAgentCancel({ run_id: agentRun.run_id })
    .catch(reportError(onStatus));
}

async function sendChatMessage(params: {
  client: WasmClient | null;
  draft: string;
  currentSessionId: string | null;
  sessions: ChatSessionSummary[];
  agentRun: AgentRun | null;
  busy: boolean;
  operation: AgentOperation;
  targetPath: string;
  snapshot: ChatSnapshot | null;
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
    currentSessionId: string | null;
    sessions: ChatSessionSummary[];
    operation: AgentOperation;
    targetPath: string;
    snapshot: ChatSnapshot | null;
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
  await client.dispatchChatSend({
    session_id: sessionId,
    content,
    related_files: [],
  });
  params.setDraft("");
  const confirmed =
    params.operation === "execute"
      ? buildCadQueryConfirmation(params.snapshot, params.targetPath, content)
      : null;
  await client.dispatchAgentInvoke({
    session_id: sessionId,
    prompt: content,
    operation: params.operation,
    confirmed_cadquery: confirmed,
  });
  await client.dispatchChatHistory({ session_id: sessionId, limit: 100 });
}

function unwrapPayload(response: unknown): unknown {
  if (!response || typeof response !== "object") return response;
  const record = response as Record<string, unknown>;
  return record["payload"] ?? response;
}

function recentAgentEvents(events: AgentEvent[]): AgentEvent[] {
  return events.filter((event) => event.event.startsWith("agent.")).slice(-6);
}

function lastAgentDoneKey(events: AgentEvent[]): string | null {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const event = events[index];
    if (event.event !== "agent.done") continue;
    const runId = stringField(event.payload ?? {}, "run_id");
    return runId || `done-${index}`;
  }
  return null;
}

function agentEventDetail(event: AgentEvent): string {
  const payload = event.payload ?? {};
  if (event.event === "agent.token") return stringField(payload, "text");
  if (event.event === "agent.error") return stringField(payload, "message");
  if (event.event === "agent.tool_start")
    return stringField(payload, "tool_name");
  if (event.event === "agent.tool_result")
    return stringField(payload, "result_json");
  if (event.event === "agent.mesh_ready") return "mesh ready";
  if (event.event === "agent.done") {
    return payload["cancelled"] === true ? "cancelled" : "done";
  }
  return event.event;
}

function stringField(payload: Record<string, unknown>, key: string): string {
  const value = payload[key];
  return typeof value === "string" ? value : "";
}

function reportError(onStatus?: (message: string) => void) {
  return (err: unknown) => {
    onStatus?.(err instanceof Error ? err.message : String(err));
  };
}

function formatTime(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "";
  return new Date(value).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

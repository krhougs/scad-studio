import { memo, useEffect, useMemo, useRef, useState } from "react";
import { Stop } from "@phosphor-icons/react";
import type {
  AgentMode,
  AgentModelRegistryModel as ProtocolAgentModelRegistryModel,
  AgentModelRegistryProvider as ProtocolAgentModelRegistryProvider,
  AgentModelRegistryResponse,
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
  agent_model_registry?: AgentModelRegistry | null;
};

export type AgentModelRegistry = AgentModelRegistryResponse;
export type AgentModelRegistryProvider = ProtocolAgentModelRegistryProvider;
export type AgentModelRegistryModel = ProtocolAgentModelRegistryModel;

export type AgentModelSelection = {
  provider_id: string;
  model_id: string;
  reasoning_effort: string | null;
  service_label: string | null;
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

type AgentModelParamPatch = {
  reasoning_effort?: string | null;
  service_label?: string | null;
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
        agentModelRegistry={controller.agentModelRegistry}
        modelControlsDisabled={controller.modelControlsDisabled}
        onNew={controller.createSession}
        onSelect={controller.selectSession}
        onCancel={controller.cancelAgent}
        onModelSelect={controller.selectAgentModel}
        onParamsUpdate={controller.updateAgentModelParams}
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
  const [modelBusy, setModelBusy] = useState(false);
  const [removedRefs, setRemovedRefs] = useState<Set<string>>(new Set());
  const sessions = snapshot?.chat_sessions ?? [];
  const snapshotCurrentSessionId = snapshot?.current_chat_session ?? null;
  const currentSessionId =
    snapshotCurrentSessionId ?? sessions[0]?.session_id ?? null;
  const messages = snapshot?.current_chat_history ?? [];
  const agentRun = snapshot?.agent_run ?? null;
  const agentModelRegistry = snapshot?.agent_model_registry ?? null;
  const agentModelSelection = useMemo(
    () => activeAgentModelSelection(agentModelRegistry),
    [agentModelRegistry],
  );
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
  useInitialAgentModelRegistry(client, onStatus);

  const actions = useChatActions({
    client,
    sessions,
    currentSessionId,
    agentRun,
    busy,
    mode,
    contextPills,
    agentModelSelection,
    onStatus,
    setBusy,
    setModelBusy,
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
    modelControlsDisabled: !client || busy || modelBusy || Boolean(agentRun),
    agentModelRegistry,
    agentModelSelection,
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

function useInitialAgentModelRegistry(
  client: WasmClient | null,
  onStatus: ((message: string) => void) | undefined,
) {
  useEffect(() => {
    if (!client) return;
    client.dispatchAgentModelRegistry().catch(reportError(onStatus));
  }, [client, onStatus]);
}

function useChatActions(input: {
  client: WasmClient | null;
  sessions: ChatSessionSummary[];
  currentSessionId: string | null;
  agentRun: AgentRun | null;
  busy: boolean;
  mode: AgentMode;
  contextPills: ContextPill[];
  agentModelSelection: AgentModelSelection | null;
  onStatus?: (message: string) => void;
  setBusy: (value: boolean) => void;
  setModelBusy: (value: boolean) => void;
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
    selectAgentModel: (value: string) =>
      void selectAgentModel(input.client, value, input.onStatus, input.setModelBusy),
    updateAgentModelParams: (patch: AgentModelParamPatch) =>
      void updateAgentModelParams(
        input.client,
        input.agentModelSelection,
        patch,
        input.onStatus,
        input.setModelBusy,
      ),
  };
}

function ChatHeader(props: {
  sessions: ChatSessionSummary[];
  currentSessionId: string | null;
  disabled: boolean;
  agentRun: AgentRun | null;
  llmConfigured: boolean;
  agentProvider: AgentProviderCapabilities | null;
  agentModelRegistry: AgentModelRegistry | null;
  modelControlsDisabled: boolean;
  onNew: () => void;
  onSelect: (id: string) => void;
  onCancel: () => void;
  onModelSelect: (value: string) => void;
  onParamsUpdate: (patch: AgentModelParamPatch) => void;
}) {
  const active = props.sessions.find(
    (session) => session.session_id === props.currentSessionId,
  );
  return (
    <header className="chat-head">
      <div>
        <div className="chat-head-main">
          <div className="title">
            budn&apos; agent{" "}
            <span
              className={props.llmConfigured ? "llm-dot llm-dot--ok" : "llm-dot llm-dot--off"}
              title={props.llmConfigured ? "AI connected" : "AI not configured"}
            />
          </div>
          <AgentModelControls
            registry={props.agentModelRegistry}
            disabled={props.modelControlsDisabled}
            onModelSelect={props.onModelSelect}
            onParamsUpdate={props.onParamsUpdate}
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

function AgentModelControls(props: {
  registry: AgentModelRegistry | null;
  disabled: boolean;
  onModelSelect: (value: string) => void;
  onParamsUpdate: (patch: AgentModelParamPatch) => void;
}) {
  const active = activeAgentModel(props.registry);
  if (!props.registry || !active) {
    return <div className="agent-model-status">model registry unavailable</div>;
  }
  return (
    <div className="agent-model-controls">
      <div className="agent-model-row">
        <select
          aria-label="agent model"
          className="agent-model-select"
          disabled={props.disabled}
          value={modelOptionValue(active.provider.id, active.model.id)}
          onChange={(event) => props.onModelSelect(event.target.value)}
        >
          {props.registry.providers.flatMap((provider) =>
            provider.models.map((model) => (
              <option
                key={modelOptionValue(provider.id, model.id)}
                value={modelOptionValue(provider.id, model.id)}
              >
                {modelOptionLabel(provider, model)}
              </option>
            )),
          )}
        </select>
        <AgentParamSelects
          registry={props.registry}
          disabled={props.disabled}
          onParamsUpdate={props.onParamsUpdate}
        />
      </div>
      <AgentModelStatus registry={props.registry} active={active} />
    </div>
  );
}

function AgentParamSelects(props: {
  registry: AgentModelRegistry;
  disabled: boolean;
  onParamsUpdate: (patch: AgentModelParamPatch) => void;
}) {
  return (
    <>
      <select
        aria-label="reasoning effort"
        className="agent-param-select"
        disabled={props.disabled || props.registry.reasoning_effort_options.length === 0}
        value={props.registry.active_reasoning_effort ?? ""}
        onChange={(event) => props.onParamsUpdate({ reasoning_effort: event.target.value || null })}
      >
        <option value="">provider default</option>
        {props.registry.reasoning_effort_options.map((option) => (
          <option key={option} value={option}>{option}</option>
        ))}
      </select>
      <select
        aria-label="service label"
        className="agent-param-select"
        disabled={props.disabled || props.registry.service_label_options.length === 0}
        value={props.registry.active_service_label ?? ""}
        onChange={(event) => props.onParamsUpdate({ service_label: event.target.value || null })}
      >
        <option value="">none</option>
        {props.registry.service_label_options.map((option) => (
          <option key={option} value={option}>{option}</option>
        ))}
      </select>
    </>
  );
}

function AgentModelStatus(props: {
  registry: AgentModelRegistry;
  active: ActiveAgentModel;
}) {
  const failedDiscovery = props.registry.providers.find(
    (provider) => provider.discovery.status === "failed",
  );
  const activeWebSearchUnsupported =
    props.active.model.native_web_search_enabled &&
    !props.active.model.native_web_search_applied;
  return (
    <div className="agent-model-status">
      <span>{modelSourceLabel(props.active.model.source)}</span>
      <span>{webSearchStateLabel(props.active.model)}</span>
      {!props.registry.active_reasoning_effort_applied ? (
        <span>reasoning not applied</span>
      ) : null}
      {!props.registry.active_service_label_applied ? (
        <span>service label not applied</span>
      ) : null}
      {failedDiscovery ? (
        <span>
          discovery failed
          {failedDiscovery.discovery.error ? `: ${failedDiscovery.discovery.error}` : ""}
        </span>
      ) : null}
      {activeWebSearchUnsupported ? (
        <span>switch model or update agents.toml / BUDN_AGENT_CONFIG</span>
      ) : null}
    </div>
  );
}

async function selectAgentModel(
  client: WasmClient | null,
  value: string,
  onStatus: ((message: string) => void) | undefined,
  setModelBusy: (value: boolean) => void,
): Promise<void> {
  const [providerId, modelId] = parseModelOptionValue(value);
  if (!client || !providerId || !modelId) return;
  setModelBusy(true);
  try {
    await client.dispatchAgentModelSelect({ provider_id: providerId, model_id: modelId });
  } catch (err) {
    reportError(onStatus)(err);
  } finally {
    setModelBusy(false);
  }
}

async function updateAgentModelParams(
  client: WasmClient | null,
  selection: AgentModelSelection | null,
  patch: AgentModelParamPatch,
  onStatus: ((message: string) => void) | undefined,
  setModelBusy: (value: boolean) => void,
): Promise<void> {
  if (!client || !selection) return;
  setModelBusy(true);
  try {
    const reasoningEffort = Object.hasOwn(patch, "reasoning_effort")
      ? patch.reasoning_effort ?? null
      : selection.reasoning_effort;
    const serviceLabel = Object.hasOwn(patch, "service_label")
      ? patch.service_label ?? null
      : selection.service_label;
    await client.dispatchAgentModelParamsUpdate({
      provider_id: selection.provider_id,
      model_id: selection.model_id,
      reasoning_effort: reasoningEffort,
      service_label: serviceLabel,
    });
  } catch (err) {
    reportError(onStatus)(err);
  } finally {
    setModelBusy(false);
  }
}

type ActiveAgentModel = {
  provider: AgentModelRegistryProvider;
  model: AgentModelRegistryModel;
};

function activeAgentModel(registry: AgentModelRegistry | null): ActiveAgentModel | null {
  if (!registry) return null;
  for (const provider of registry.providers) {
    if (provider.id !== registry.active_provider_id) continue;
    const model = provider.models.find((item) => item.id === registry.active_model_id);
    return model ? { provider, model } : null;
  }
  return null;
}

function activeAgentModelSelection(
  registry: AgentModelRegistry | null,
): AgentModelSelection | null {
  if (!activeAgentModel(registry) || !registry) return null;
  return {
    provider_id: registry.active_provider_id,
    model_id: registry.active_model_id,
    reasoning_effort: registry.active_reasoning_effort,
    service_label: registry.active_service_label,
  };
}

function modelOptionValue(providerId: string, modelId: string): string {
  return `${providerId}/${modelId}`;
}

function parseModelOptionValue(value: string): [string | null, string | null] {
  const slash = value.indexOf("/");
  if (slash <= 0 || slash === value.length - 1) return [null, null];
  return [value.slice(0, slash), value.slice(slash + 1)];
}

function modelOptionLabel(
  provider: AgentModelRegistryProvider,
  model: AgentModelRegistryModel,
): string {
  return `${provider.label ?? provider.id} / ${model.label ?? model.id} · ${model.source}`;
}

function modelSourceLabel(source: AgentModelRegistryModel["source"]): string {
  if (source === "discovered_with_override") return "source: discovered override";
  return `source: ${source}`;
}

function webSearchStateLabel(model: AgentModelRegistryModel): string {
  if (!model.native_web_search_enabled) return "web search off";
  if (model.native_web_search_applied) return "web search active";
  const reason = model.web_search_unsupported_reason;
  return reason ? `web search unavailable: ${reason}` : "web search unavailable";
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

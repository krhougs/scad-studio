import { describe, expect, it, beforeEach } from "vitest";
import {
  useProtocolStore,
  chatSessionsEqual,
  chatHistoryEqual,
  agentRunEqual,
  agentEventsEqual,
  agentProviderEqual,
  cadQueryResultsEqual,
} from "../../src/state/protocol-store";
import type {
  AgentEvent,
  AgentModelRegistry,
  AgentRun,
  ChatMessageRecord,
  ChatSessionSummary,
} from "../../src/workbench/chat-zone";

function makeSession(overrides: Partial<ChatSessionSummary> & { session_id: string }): ChatSessionSummary {
  return { title: "test", archived: false, message_count: 0, ...overrides };
}

function makeRecord(id: string, role: ChatMessageRecord["role"] = "user"): ChatMessageRecord {
  return { message_id: id, ts_ms: 1000, role, content: "", tool_calls: [], tool_result: null, mesh_result: null };
}

function makeEvent(event: string, payload?: Record<string, unknown>): AgentEvent {
  return { event, payload };
}

describe("applySnapshot", () => {
  beforeEach(() => {
    useProtocolStore.setState({
      workspace_current: null,
      chat_sessions: [],
      current_chat_session: null,
      current_chat_history: [],
      agent_run: null,
      agent_events: [],
      current_selection: null,
      cadquery_results: [],
      llm_configured: true,
      agent_provider: null,
      agent_model_registry: null,
      transport_status: null,
    });
  });

  it("updates workspace fields when changed", () => {
    const { applySnapshot } = useProtocolStore.getState();
    applySnapshot({ workspace_current: { root_name: "my-project", workspace_id: "w1" } });
    const state = useProtocolStore.getState();
    expect(state.workspace_current?.root_name).toBe("my-project");
  });

  it("does not update workspace when values are identical", () => {
    const initial = { root_name: "proj", workspace_id: "w1" };
    useProtocolStore.setState({ workspace_current: initial });
    const before = useProtocolStore.getState().workspace_current;

    const { applySnapshot } = useProtocolStore.getState();
    applySnapshot({ workspace_current: { root_name: "proj", workspace_id: "w1" } });
    const after = useProtocolStore.getState().workspace_current;
    expect(after).toBe(before);
  });

  it("updates chat_sessions when content changes", () => {
    const { applySnapshot } = useProtocolStore.getState();
    const sessions = [makeSession({ session_id: "s1", title: "hello" })];
    applySnapshot({ chat_sessions: sessions });
    expect(useProtocolStore.getState().chat_sessions).toEqual(sessions);
  });

  it("does not update chat_sessions when structurally equal", () => {
    const sessions = [makeSession({ session_id: "s1", title: "hello" })];
    useProtocolStore.setState({ chat_sessions: sessions });
    const before = useProtocolStore.getState().chat_sessions;

    const { applySnapshot } = useProtocolStore.getState();
    applySnapshot({ chat_sessions: [makeSession({ session_id: "s1", title: "hello" })] });
    expect(useProtocolStore.getState().chat_sessions).toBe(before);
  });

  it("updates agent_run when changed", () => {
    const { applySnapshot } = useProtocolStore.getState();
    applySnapshot({ agent_run: { session_id: "s1", run_id: "r1" } });
    expect(useProtocolStore.getState().agent_run).toEqual({ session_id: "s1", run_id: "r1" });
  });

  it("does not update agent_run when structurally equal", () => {
    const run: AgentRun = { session_id: "s1", run_id: "r1" };
    useProtocolStore.setState({ agent_run: run });
    const before = useProtocolStore.getState().agent_run;

    const { applySnapshot } = useProtocolStore.getState();
    applySnapshot({ agent_run: { session_id: "s1", run_id: "r1" } });
    expect(useProtocolStore.getState().agent_run).toBe(before);
  });

  it("updates current_chat_history when message_id differs", () => {
    const history = [makeRecord("m1")];
    useProtocolStore.setState({ current_chat_history: history });

    const { applySnapshot } = useProtocolStore.getState();
    applySnapshot({ current_chat_history: [makeRecord("m1"), makeRecord("m2")] });
    expect(useProtocolStore.getState().current_chat_history).toHaveLength(2);
  });

  it("does not update history when message_ids and timestamps match", () => {
    const history = [makeRecord("m1"), makeRecord("m2")];
    useProtocolStore.setState({ current_chat_history: history });
    const before = useProtocolStore.getState().current_chat_history;

    const { applySnapshot } = useProtocolStore.getState();
    applySnapshot({ current_chat_history: [makeRecord("m1"), makeRecord("m2")] });
    expect(useProtocolStore.getState().current_chat_history).toBe(before);
  });

  it("updates agent_events when length changes", () => {
    const { applySnapshot } = useProtocolStore.getState();
    applySnapshot({ agent_events: [makeEvent("agent.token", { text: "hi" })] });
    expect(useProtocolStore.getState().agent_events).toHaveLength(1);
  });

  it("does not update agent_events when length is same", () => {
    const events = [makeEvent("agent.token", { text: "hi" })];
    useProtocolStore.setState({ agent_events: events });
    const before = useProtocolStore.getState().agent_events;

    const { applySnapshot } = useProtocolStore.getState();
    applySnapshot({ agent_events: [makeEvent("agent.token", { text: "world" })] });
    expect(useProtocolStore.getState().agent_events).toBe(before);
  });

  it("handles null/undefined snapshot gracefully", () => {
    const { applySnapshot } = useProtocolStore.getState();
    applySnapshot(null);
    applySnapshot(undefined);
    expect(useProtocolStore.getState().workspace_current).toBeNull();
  });

  it("only patches changed domains", () => {
    const sessions = [makeSession({ session_id: "s1" })];
    useProtocolStore.setState({ chat_sessions: sessions });
    const beforeSessions = useProtocolStore.getState().chat_sessions;

    const { applySnapshot } = useProtocolStore.getState();
    applySnapshot({
      workspace_current: { root_name: "new-name" },
      chat_sessions: [makeSession({ session_id: "s1" })],
    });

    expect(useProtocolStore.getState().workspace_current?.root_name).toBe("new-name");
    expect(useProtocolStore.getState().chat_sessions).toBe(beforeSessions);
  });

  it("updates transport_status", () => {
    const { applySnapshot } = useProtocolStore.getState();
    applySnapshot({ transport_status: "connected" });
    expect(useProtocolStore.getState().transport_status).toBe("connected");
  });

  it("updates llm_configured", () => {
    const { applySnapshot } = useProtocolStore.getState();
    applySnapshot({ llm_configured: false });
    expect(useProtocolStore.getState().llm_configured).toBe(false);
  });

  it("updates agent_provider capability", () => {
    const { applySnapshot } = useProtocolStore.getState();
    applySnapshot({
      agent_provider: {
        provider: "openai_responses",
        model: "gpt-5.2",
        native_web_search_enabled: true,
        search_sources_supported: false,
      },
    });
    expect(useProtocolStore.getState().agent_provider).toEqual({
      provider: "openai_responses",
      model: "gpt-5.2",
      native_web_search_enabled: true,
      search_sources_supported: false,
    });
  });

  it("updates agent_model_registry and preserves equal snapshots", () => {
    const { applySnapshot } = useProtocolStore.getState();
    const registry = makeAgentModelRegistry();
    applySnapshot({ agent_model_registry: registry });
    expect(useProtocolStore.getState().agent_model_registry).toEqual(registry);
    const before = useProtocolStore.getState().agent_model_registry;

    applySnapshot({ agent_model_registry: makeAgentModelRegistry() });
    expect(useProtocolStore.getState().agent_model_registry).toBe(before);
  });

  it("updates cadquery_results with artifact relation", () => {
    const { applySnapshot } = useProtocolStore.getState();
    const results = [makeCadQueryReady("cq_1")];
    applySnapshot({ cadquery_results: results });
    expect(useProtocolStore.getState().cadquery_results).toEqual(results);
  });

  it("does not clear existing fields when snapshot omits them", () => {
    useProtocolStore.setState({
      workspace_current: { root_name: "proj", workspace_id: "w1" },
      chat_sessions: [makeSession({ session_id: "s1" })],
      agent_provider: {
        provider: "openai_responses",
        model: "gpt-5.2",
        native_web_search_enabled: true,
        search_sources_supported: false,
      },
      agent_model_registry: makeAgentModelRegistry(),
      transport_status: "connected",
    });
    const { applySnapshot } = useProtocolStore.getState();
    applySnapshot({ llm_configured: false });
    const state = useProtocolStore.getState();
    expect(state.workspace_current).toBeNull();
    expect(state.chat_sessions).toEqual([]);
    expect(state.agent_provider).toBeNull();
    expect(state.agent_model_registry).toBeNull();
    expect(state.transport_status).toBeNull();
  });
});

describe("chatSessionsEqual", () => {
  it("returns true for same reference", () => {
    const arr: ChatSessionSummary[] = [];
    expect(chatSessionsEqual(arr, arr)).toBe(true);
  });

  it("returns false for different length", () => {
    expect(chatSessionsEqual([], [makeSession({ session_id: "s1" })])).toBe(false);
  });

  it("returns true for structurally equal sessions", () => {
    const a = [makeSession({ session_id: "s1", title: "a", message_count: 3 })];
    const b = [makeSession({ session_id: "s1", title: "a", message_count: 3 })];
    expect(chatSessionsEqual(a, b)).toBe(true);
  });

  it("returns false when title differs", () => {
    const a = [makeSession({ session_id: "s1", title: "a" })];
    const b = [makeSession({ session_id: "s1", title: "b" })];
    expect(chatSessionsEqual(a, b)).toBe(false);
  });
});

describe("chatHistoryEqual", () => {
  it("returns true for same reference", () => {
    const arr: ChatMessageRecord[] = [];
    expect(chatHistoryEqual(arr, arr)).toBe(true);
  });

  it("returns false for different length", () => {
    expect(chatHistoryEqual([], [makeRecord("m1")])).toBe(false);
  });

  it("returns true when message_id and ts_ms match", () => {
    expect(chatHistoryEqual([makeRecord("m1")], [makeRecord("m1")])).toBe(true);
  });

  it("returns false when message_id differs", () => {
    expect(chatHistoryEqual([makeRecord("m1")], [makeRecord("m2")])).toBe(false);
  });

  it("returns false when search sources differ for the same message", () => {
    expect(chatHistoryEqual(
      [makeRecord("m1", "assistant")],
      [{
        ...makeRecord("m1", "assistant"),
        search_sources: [
          {
            title: "OpenAI web search",
            url: "https://developers.openai.com/api/docs/guides/tools-web-search",
          },
        ],
      }],
    )).toBe(false);
  });
});

describe("agentRunEqual", () => {
  it("returns true for both null", () => {
    expect(agentRunEqual(null, null)).toBe(true);
  });

  it("returns false for null vs non-null", () => {
    expect(agentRunEqual(null, { session_id: "s", run_id: "r" })).toBe(false);
  });

  it("returns true for same fields", () => {
    expect(agentRunEqual(
      { session_id: "s", run_id: "r" },
      { session_id: "s", run_id: "r" },
    )).toBe(true);
  });

  it("returns false when run_id differs", () => {
    expect(agentRunEqual(
      { session_id: "s", run_id: "r1" },
      { session_id: "s", run_id: "r2" },
    )).toBe(false);
  });
});

describe("agentEventsEqual", () => {
  it("returns true for same reference", () => {
    const arr: AgentEvent[] = [];
    expect(agentEventsEqual(arr, arr)).toBe(true);
  });

  it("returns true for same length and same last event", () => {
    expect(agentEventsEqual(
      [makeEvent("agent.token")],
      [makeEvent("agent.token")],
    )).toBe(true);
  });

  it("returns false when last event type differs", () => {
    expect(agentEventsEqual(
      [makeEvent("agent.token")],
      [makeEvent("agent.done")],
    )).toBe(false);
  });

  it("returns true for empty arrays", () => {
    expect(agentEventsEqual([], [])).toBe(true);
  });

  it("returns false for different length", () => {
    expect(agentEventsEqual([], [makeEvent("agent.token")])).toBe(false);
  });
});

describe("agentProviderEqual", () => {
  it("returns true for same provider capability", () => {
    expect(agentProviderEqual(
      {
        provider: "openai_responses",
        model: "gpt-5.2",
        native_web_search_enabled: true,
        search_sources_supported: false,
      },
      {
        provider: "openai_responses",
        model: "gpt-5.2",
        native_web_search_enabled: true,
        search_sources_supported: false,
      },
    )).toBe(true);
  });

  it("returns false when web search capability differs", () => {
    expect(agentProviderEqual(
      {
        provider: "openai_responses",
        model: "gpt-5.2",
        native_web_search_enabled: true,
        search_sources_supported: false,
      },
      {
        provider: "openai_responses",
        model: "gpt-5.2",
        native_web_search_enabled: false,
        search_sources_supported: false,
      },
    )).toBe(false);
  });
});

describe("cadQueryResultsEqual", () => {
  it("compares artifact relation fields", () => {
    expect(
      cadQueryResultsEqual([makeCadQueryReady("cq_1")], [makeCadQueryReady("cq_1")]),
    ).toBe(true);
    expect(
      cadQueryResultsEqual(
        [makeCadQueryReady("cq_1", "outputs/model.step")],
        [makeCadQueryReady("cq_1", "outputs/other.step")],
      ),
    ).toBe(false);
  });
});

function makeCadQueryReady(resultId: string, exportPath = "outputs/model.step") {
  return {
    result_id: resultId,
    build_id: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    part_count: 1,
    face_count: 1,
    edge_count: 0,
    vertex_count: 0,
    artifact_relation: {
      source_path: "parts/model.py",
      exports: [
        {
          name: "step",
          path: exportPath,
          hash: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        },
      ],
    },
  };
}

function makeAgentModelRegistry(): AgentModelRegistry {
  return {
    active_provider_id: "openai",
    active_model_id: "gpt-5.2",
    active_reasoning_effort: "high",
    active_reasoning_effort_applied: true,
    active_service_label: "flex",
    active_service_label_applied: true,
    reasoning_effort_options: ["low", "medium", "high"],
    service_label_options: ["default", "flex"],
    providers: [
      {
        id: "openai",
        kind: "openai_responses",
        label: "OpenAI",
        discovery: { enabled: true, status: "succeeded", error: null },
        models: [
          {
            id: "gpt-5.2",
            label: "GPT 5.2",
            source: "discovered_with_override",
            reasoning_effort: "high",
            service_label: "flex",
            native_web_search_enabled: true,
            native_web_search_applied: true,
            web_search_supported: true,
            web_search_unsupported_reason: null,
            search_sources_supported: false,
          },
        ],
      },
    ],
  };
}

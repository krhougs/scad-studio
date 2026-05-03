import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useProtocolStore } from "../../src/state/protocol-store";
import { runSavedPlan, sendChatMessage } from "../../src/workbench/chat-actions";
import { ChatZone, type ChatSnapshot } from "../../src/workbench/chat-zone";
import type { WasmClient } from "../../src/wasm-bridge";

const STORE_DEFAULTS = {
  chat_sessions: [] as ChatSnapshot["chat_sessions"],
  current_chat_session: null,
  current_chat_history: [] as ChatSnapshot["current_chat_history"],
  agent_run: null,
  agent_runtime_status: null,
  agent_events: [] as ChatSnapshot["agent_events"],
  current_selection: null,
  llm_configured: true,
  workspace_current: null,
  transport_status: null,
};

function setSnapshot(snapshot: Record<string, unknown>) {
  useProtocolStore.setState({ ...STORE_DEFAULTS, ...snapshot });
}

describe("ChatZone", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
    useProtocolStore.setState({ ...STORE_DEFAULTS });
  });

  it("sends agent invoke without plan_ref", async () => {
    const client = fakeClient();
    setSnapshot(chatSnapshot());
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    const user = userEvent.setup();
    await user.type(screen.getByTestId("chat-input"), "make the lid taller");
    await user.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => expect(client.dispatchAgentStartTurn).toHaveBeenCalled());
    expect(client.dispatchAgentStartTurn).toHaveBeenCalledWith(
      expect.objectContaining({
        agent_id: "agent-main",
        mode: "agent",
        plan_ref: null,
        context_refs: [],
      }),
    );
  });

  it("renders provider models, discovery status, and web search state", () => {
    const client = fakeClient();
    setSnapshot(chatSnapshot(null, agentModelRegistry()));
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    const modelSelect = screen.getByLabelText("agent model") as HTMLSelectElement;
    expect(modelSelect.value).toBe("openai/gpt-5.2");
    expect(modelSelect.textContent).toContain("GPT 5.2");
    expect(modelSelect.textContent).toContain("Claude Sonnet");
    expect(screen.getAllByText(/override/).length).toBeGreaterThan(0);
    expect(screen.getByText(/model discovery failed; using configured models/)).toBeTruthy();
    expect(screen.queryByText(/manual fallback remains available/)).toBeNull();
    expect(screen.getByText(/web search active/)).toBeTruthy();
    expect(screen.queryByText(/web search unavailable/)).toBeNull();
  });

  it("dispatches model and parameter updates from the chat header", async () => {
    const client = fakeClient();
    setSnapshot(chatSnapshot(null, agentModelRegistry()));
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    fireEvent.change(screen.getByLabelText("agent model"), {
      target: { value: "anthropic/claude-sonnet" },
    });
    await waitFor(() => {
      expect(client.dispatchAgentModelSelect).toHaveBeenCalledWith({
        provider_id: "anthropic",
        model_id: "claude-sonnet",
      });
    });

    fireEvent.change(screen.getByLabelText("reasoning effort"), {
      target: { value: "medium" },
    });
    await waitFor(() => {
      expect(client.dispatchAgentModelParamsUpdate).toHaveBeenCalledWith({
        provider_id: "openai",
        model_id: "gpt-5.2",
        reasoning_effort: "medium",
        service_label: "flex",
      });
    });

    fireEvent.change(screen.getByLabelText("service label"), {
      target: { value: "default" },
    });
    await waitFor(() => {
      expect(client.dispatchAgentModelParamsUpdate).toHaveBeenCalledWith({
        provider_id: "openai",
        model_id: "gpt-5.2",
        reasoning_effort: "high",
        service_label: "default",
      });
    });

    fireEvent.change(screen.getByLabelText("reasoning effort"), {
      target: { value: "" },
    });
    await waitFor(() => {
      expect(client.dispatchAgentModelParamsUpdate).toHaveBeenCalledWith({
        provider_id: "openai",
        model_id: "gpt-5.2",
        reasoning_effort: null,
        service_label: "flex",
      });
    });
  });

  it("disables model controls while a model switch request is pending", async () => {
    let resolveSelect: (() => void) | undefined;
    const client = fakeClient();
    client.dispatchAgentModelSelect = vi.fn().mockReturnValueOnce(
      new Promise<void>((resolve) => {
        resolveSelect = resolve;
      }),
    );
    setSnapshot(chatSnapshot(null, agentModelRegistry()));
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    const modelSelect = screen.getByLabelText("agent model") as HTMLSelectElement;
    fireEvent.change(modelSelect, {
      target: { value: "anthropic/claude-sonnet" },
    });

    await waitFor(() => expect(modelSelect.disabled).toBe(true));
    resolveSelect?.();
    await waitFor(() => expect(modelSelect.disabled).toBe(false));
  });

  it("keeps model controls read-only when the current chat has a bound model", async () => {
    const client = fakeClient();
    const snapshot = chatSnapshot(null, agentModelRegistry());
    snapshot.chat_sessions = [
      {
        ...snapshot.chat_sessions![0]!,
        bound_model: agentModelSelection(),
      },
    ];
    setSnapshot(snapshot);
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    expect((screen.getByLabelText("agent model") as HTMLSelectElement).disabled).toBe(true);
    expect((screen.getByLabelText("reasoning effort") as HTMLSelectElement).disabled).toBe(true);
    expect((screen.getByLabelText("service label") as HTMLSelectElement).disabled).toBe(true);
  });

  it("keeps bound null params and unavailable model visible without active fallback", async () => {
    const client = fakeClient();
    const snapshot = chatSnapshot(null, agentModelRegistry());
    snapshot.chat_sessions = [
      {
        ...snapshot.chat_sessions![0]!,
        bound_model: {
          provider_id: "missing",
          provider_type: "openai_responses",
          model_id: "retired-model",
          reasoning_effort: null,
          service_label: null,
        },
      },
    ];
    setSnapshot(snapshot);
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    const modelSelect = screen.getByLabelText("agent model") as HTMLSelectElement;
    expect(modelSelect.disabled).toBe(true);
    expect(modelSelect.value).toBe("missing/retired-model");
    expect((screen.getByLabelText("reasoning effort") as HTMLSelectElement).value).toBe("");
    expect((screen.getByLabelText("service label") as HTMLSelectElement).value).toBe("");
    expect(screen.getByText("bound model unavailable")).toBeTruthy();
  });

  it("shows raw bound model when the registry is unavailable", async () => {
    const client = fakeClient();
    const snapshot = chatSnapshot(null, null);
    snapshot.chat_sessions = [
      {
        ...snapshot.chat_sessions![0]!,
        bound_model: {
          provider_id: "openai",
          provider_type: "openai_responses",
          model_id: "gpt-5.2",
          reasoning_effort: "xhigh",
          service_label: "batch",
        },
      },
    ];
    setSnapshot(snapshot);
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    expect((screen.getByLabelText("agent model") as HTMLSelectElement).value).toBe("openai/gpt-5.2");
    expect((screen.getByLabelText("reasoning effort") as HTMLSelectElement).value).toBe("xhigh");
    expect((screen.getByLabelText("service label") as HTMLSelectElement).value).toBe("batch");
  });

  it("shows raw bound params when registry options no longer contain them", async () => {
    const client = fakeClient();
    const snapshot = chatSnapshot(null, agentModelRegistry());
    snapshot.chat_sessions = [
      {
        ...snapshot.chat_sessions![0]!,
        bound_model: {
          provider_id: "openai",
          provider_type: "openai_responses",
          model_id: "gpt-5.2",
          reasoning_effort: "xhigh",
          service_label: "batch",
        },
      },
    ];
    setSnapshot(snapshot);
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    expect((screen.getByLabelText("reasoning effort") as HTMLSelectElement).value).toBe("xhigh");
    expect((screen.getByLabelText("service label") as HTMLSelectElement).value).toBe("batch");
  });

  it("does not show active model applied warnings for a bound model", async () => {
    const client = fakeClient();
    const registry = agentModelRegistry();
    registry.active_reasoning_effort_applied = false;
    registry.active_service_label_applied = false;
    const snapshot = chatSnapshot(null, registry);
    snapshot.chat_sessions = [
      {
        ...snapshot.chat_sessions![0]!,
        bound_model: agentModelSelection(),
      },
    ];
    setSnapshot(snapshot);
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    expect(screen.getByText("bound model")).toBeTruthy();
    expect(screen.queryByText("reasoning not applied")).toBeNull();
    expect(screen.queryByText("service label not applied")).toBeNull();
  });

  it("keeps null service label and shows active web search downgrade", async () => {
    const client = fakeClient();
    const registry = agentModelRegistry();
    registry.active_provider_id = "anthropic";
    registry.active_model_id = "claude-sonnet";
    registry.active_reasoning_effort = "medium";
    registry.active_service_label = null;
    registry.active_service_label_applied = false;
    setSnapshot(chatSnapshot(null, registry));
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    expect((screen.getByLabelText("agent model") as HTMLSelectElement).value)
      .toBe("anthropic/claude-sonnet");
    expect((screen.getByLabelText("service label") as HTMLSelectElement).value)
      .toBe("");
    expect(screen.getByText(/web search unavailable for selected model/)).toBeTruthy();
    expect(screen.queryByText(/model does not support web search/)).toBeNull();
    expect(screen.queryByText(/agents.toml/)).toBeNull();
    expect(screen.queryByText(/BUDN_AGENT_CONFIG/)).toBeNull();

    fireEvent.change(screen.getByLabelText("reasoning effort"), {
      target: { value: "low" },
    });
    await waitFor(() => {
      expect(client.dispatchAgentModelParamsUpdate).toHaveBeenCalledWith({
        provider_id: "anthropic",
        model_id: "claude-sonnet",
        reasoning_effort: "low",
        service_label: null,
      });
    });
  });

  it("uses agent_id for later turns after a chat is already bound", async () => {
    const client = fakeClient();
    setSnapshot(chatSnapshot(null, agentModelRegistry()));
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    const user = userEvent.setup();
    await user.type(screen.getByTestId("chat-input"), "make the lid taller");
    await user.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => expect(client.dispatchAgentStartTurn).toHaveBeenCalled());
    expect(client.dispatchAgentStartTurn).toHaveBeenCalledWith(
      expect.objectContaining({
        agent_id: "agent-main",
        prompt: "make the lid taller",
      }),
    );
    expect(client.dispatchAgentInvoke).not.toHaveBeenCalled();
  });

  it("keeps IME composition text when chat state updates during composition", async () => {
    const client = fakeClient();
    setSnapshot(chatSnapshot());
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    const input = screen.getByTestId("chat-input") as HTMLTextAreaElement;
    fireEvent.compositionStart(input);
    setNativeTextareaValue(input, "zhong");
    fireEvent.input(input, {
      data: "zhong",
      inputType: "insertCompositionText",
      isComposing: true,
    });

    act(() => {
      useProtocolStore.setState({
        agent_events: [
          {
            event: "agent.token",
            payload: {
              session_id: "main",
              run_id: "run-main",
              text: "stream update",
            },
          },
        ],
      });
    });

    expect(input.value).toBe("zhong");

    setNativeTextareaValue(input, "中");
    fireEvent.compositionEnd(input, { data: "中" });
    await userEvent.setup().click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => expect(client.dispatchAgentStartTurn).toHaveBeenCalled());
    expect(client.dispatchAgentStartTurn).toHaveBeenCalledWith(
      expect.objectContaining({
        prompt: "中",
      }),
    );
  });

  it("sends the mode selected in the composer dropdown", async () => {
    const client = fakeClient();
    setSnapshot(chatSnapshot());
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    const modeSelect = screen.getByLabelText("agent mode");
    expect((modeSelect as HTMLSelectElement).value).toBe("agent");

    const user = userEvent.setup();
    fireEvent.change(modeSelect, { target: { value: "plan" } });
    await user.type(screen.getByTestId("chat-input"), "apply the latest plan");
    await user.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => expect(client.dispatchAgentStartTurn).toHaveBeenCalled());
    expect(client.dispatchAgentStartTurn).toHaveBeenCalledWith(
      expect.objectContaining({
        mode: "plan",
        prompt: "apply the latest plan",
      }),
    );
  });

  it("lets slash commands override the composer mode dropdown", async () => {
    const client = fakeClient();
    setSnapshot(chatSnapshot());
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    const user = userEvent.setup();
    fireEvent.change(screen.getByLabelText("agent mode"), {
      target: { value: "plan" },
    });
    await user.type(screen.getByTestId("chat-input"), "/agent explain CadQuery loft");
    await user.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => expect(client.dispatchAgentStartTurn).toHaveBeenCalled());
    expect(client.dispatchAgentStartTurn).toHaveBeenCalledWith(
      expect.objectContaining({
        mode: "agent",
        prompt: "explain CadQuery loft",
      }),
    );
  });

  it("refreshes current chat history after agent done", async () => {
    const client = fakeClient();
    setSnapshot({
      ...chatSnapshot(),
      agent_events: [
        {
          event: "agent.done",
          payload: { run_id: "run-1", cancelled: false },
        },
      ],
    });
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    await waitFor(() => {
      expect(client.dispatchChatHistory).toHaveBeenCalledWith({
        session_id: "main",
        limit: 100,
      });
    });
  });

  it("refreshes current chat history after agent error", async () => {
    const client = fakeClient();
    setSnapshot(chatSnapshot());
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );
    await waitFor(() => expect(client.dispatchChatHistory).toHaveBeenCalled());
    vi.mocked(client.dispatchChatHistory).mockClear();

    act(() => {
      useProtocolStore.setState({
        agent_events: [
          {
            event: "agent.error",
            payload: {
              run_id: "run-1",
              error_type: "llm_error",
              message: "Rig Agent is not configured",
            },
          },
        ],
      });
    });

    await waitFor(() => {
      expect(client.dispatchChatHistory).toHaveBeenCalledWith({
        session_id: "main",
        limit: 100,
      });
    });
  });

  it("refreshes current chat history after recovered failed state", async () => {
    const client = fakeClient();
    setSnapshot(chatSnapshot());
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );
    await waitFor(() => expect(client.dispatchChatHistory).toHaveBeenCalled());
    vi.mocked(client.dispatchChatHistory).mockClear();

    act(() => {
      useProtocolStore.setState({
        agent_events: [
          {
            event: "agent.state_changed",
            payload: { run_id: "run-1", state: "failed" },
          },
        ],
      });
    });

    await waitFor(() => {
      expect(client.dispatchChatHistory).toHaveBeenCalledWith({
        session_id: "main",
        limit: 100,
      });
    });
  });

  it("loads the first chat history after a cold-start chat list", async () => {
    const client = fakeClient();
    setSnapshot({
      ...chatSnapshot(),
      current_chat_session: null,
      current_chat_history: [],
    });
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    await waitFor(() => {
      expect(client.dispatchChatHistory).toHaveBeenCalledWith({
        session_id: "main",
        limit: 100,
      });
    });
  });

  it("keeps New Chat as a local draft until the first message is sent", async () => {
    const client = fakeClient();
    setSnapshot(chatSnapshot());
    render(<ChatZone client={client as unknown as WasmClient} />);

    await userEvent.setup().click(screen.getByRole("button", { name: /new/i }));

    expect(client.dispatchChatCreate).not.toHaveBeenCalled();
    expect(client.dispatchChatSend).not.toHaveBeenCalled();
    expect(client.dispatchAgentInvoke).not.toHaveBeenCalled();
    expect(client.dispatchAgentStartTurn).not.toHaveBeenCalled();
    expect(screen.getAllByText("Untitled").length).toBeGreaterThan(0);
  });

  it("sends a client request id when first message creates a draft chat", async () => {
    const client = fakeClient();
    setSnapshot(chatSnapshot(null, agentModelRegistry()));
    render(<ChatZone client={client as unknown as WasmClient} />);

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /new/i }));
    await user.type(screen.getByTestId("chat-input"), "start from draft");
    await user.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => expect(client.dispatchChatCreate).toHaveBeenCalled());
    expect(client.dispatchChatCreate).toHaveBeenCalledWith(
      expect.objectContaining({
        client_request_id: expect.any(String),
        initial_user_message: "start from draft",
      }),
    );
    expect(client.dispatchChatSend).not.toHaveBeenCalled();
  });

  it("generates a client request id when first message creates chat without a local draft", async () => {
    const client = fakeClient();
    setSnapshot({
      ...chatSnapshot(null, agentModelRegistry()),
      chat_sessions: [],
      current_chat_session: null,
    });
    render(<ChatZone client={client as unknown as WasmClient} />);

    const user = userEvent.setup();
    await user.type(screen.getByTestId("chat-input"), "/plan design a sliding lid");
    await user.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => expect(client.dispatchChatCreate).toHaveBeenCalled());
    const createRequest = vi.mocked(client.dispatchChatCreate).mock
      .calls[0][0] as { client_request_id: string };
    expect(client.dispatchAgentInvoke).not.toHaveBeenCalled();
    expect(client.dispatchAgentStartTurn).not.toHaveBeenCalled();

    expect(createRequest).toEqual(
      expect.objectContaining({
        client_request_id: expect.any(String),
        initial_user_message: "design a sliding lid",
      }),
    );
    expect(createRequest).toEqual(
      expect.objectContaining({
        initial_turn: expect.objectContaining({
          mode: "plan",
          plan_ref: null,
          context_refs: [],
        }),
      }),
    );
    expect(client.dispatchChatSend).not.toHaveBeenCalled();
  });

  it("keeps a local draft visible when first send fails", async () => {
    const client = fakeClient();
    client.dispatchChatCreate = vi.fn().mockRejectedValue(new Error("create failed"));
    setSnapshot(chatSnapshot(null, agentModelRegistry()));
    render(<ChatZone client={client as unknown as WasmClient} />);

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /new/i }));
    await user.type(screen.getByTestId("chat-input"), "start from draft");
    await user.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => expect(client.dispatchChatCreate).toHaveBeenCalled());
    expect(screen.getAllByText("Untitled").length).toBeGreaterThan(0);
  });

  it("does not dispatch a second Agent command when chat create starts the first turn", async () => {
    const client = fakeClient();
    const onStatus = vi.fn();
    setSnapshot(chatSnapshot(null, agentModelRegistry()));
    render(
      <ChatZone
        client={client as unknown as WasmClient}
        onStatus={onStatus}
      />,
    );

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /new/i }));
    await user.type(screen.getByTestId("chat-input"), "start from draft");
    await user.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => expect(client.dispatchChatCreate).toHaveBeenCalled());
    await waitFor(() => expect(screen.queryByText("Untitled")).toBeNull());
    expect(client.dispatchAgentInvoke).not.toHaveBeenCalled();
    expect(client.dispatchAgentStartTurn).not.toHaveBeenCalled();
    expect(onStatus).not.toHaveBeenCalledWith("invoke failed");
  });

  it("commits the local draft when chat list refresh fails after chat create succeeds", async () => {
    const client = fakeClient();
    const onStatus = vi.fn();
    client.dispatchChatList = vi.fn().mockRejectedValue(new Error("list failed"));
    setSnapshot(chatSnapshot(null, agentModelRegistry()));
    render(
      <ChatZone
        client={client as unknown as WasmClient}
        onStatus={onStatus}
      />,
    );

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /new/i }));
    await user.type(screen.getByTestId("chat-input"), "start from draft");
    await user.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => expect(client.dispatchChatCreate).toHaveBeenCalled());
    await waitFor(() => expect(screen.queryByText("Untitled")).toBeNull());
    expect(client.dispatchAgentInvoke).not.toHaveBeenCalled();
    expect(client.dispatchAgentStartTurn).not.toHaveBeenCalled();
    expect(onStatus).toHaveBeenCalledWith("list failed");
  });

  it("keeps send busy while first chat create starts the initial turn", async () => {
    const client = fakeClient();
    let finishCreate: () => void = () => {
      throw new Error("create not started");
    };
    client.dispatchChatCreate = vi.fn(
      () =>
        new Promise((resolve) => {
          finishCreate = () => resolve({ session_id: "main" });
        }),
    );
    const setBusy = vi.fn();

    const pending = sendChatMessage({
      client: client as unknown as WasmClient,
      mode: "agent",
      currentSessionId: null,
      sessions: [],
      agentRun: null,
      busy: false,
      contextPills: [],
      agentModelSelection: agentModelSelection(),
      setBusy,
    }, "start from draft");

    await waitFor(() => expect(client.dispatchChatCreate).toHaveBeenCalled());
    expect(setBusy.mock.calls).toEqual([[true]]);

    finishCreate();
    await expect(pending).resolves.toBe(true);
    expect(setBusy.mock.calls).toEqual([[true], [false]]);
  });

  it("uses the first backend chat title when a local draft is the only session", async () => {
    const client = fakeClient();
    setSnapshot({
      ...chatSnapshot(null, agentModelRegistry()),
      chat_sessions: [],
      current_chat_session: null,
    });
    render(<ChatZone client={client as unknown as WasmClient} />);

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /new/i }));
    await user.type(screen.getByTestId("chat-input"), "start from draft");
    await user.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => expect(client.dispatchChatCreate).toHaveBeenCalled());
    expect(client.dispatchChatCreate).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "main",
      }),
    );
  });

  it("shows context pills from viewer selection and includes refs in invoke", async () => {
    const client = fakeClient();
    setSnapshot(chatSnapshot({
      selections: [faceSelection()],
      active_index: 0,
    }));
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    expect(screen.getByTestId("context-pill-bar")).toBeTruthy();
    expect(screen.getByTestId("context-pill-bar").textContent).toContain(
      "@feature[top_lid.lid_alignment_surface]",
    );

    const user = userEvent.setup();
    await user.type(screen.getByTestId("chat-input"), "open a slot on this face");
    await user.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => expect(client.dispatchAgentStartTurn).toHaveBeenCalled());
    expect(client.dispatchAgentStartTurn).toHaveBeenCalledWith(
      expect.objectContaining({
        agent_id: "agent-main",
        mode: "agent",
        plan_ref: null,
        context_refs: ["@face[top_lid:f_0]"],
      }),
    );
  });

  it("sends explicit mode when using slash command", async () => {
    const client = fakeClient();
    setSnapshot(chatSnapshot());
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    const user = userEvent.setup();
    await user.type(screen.getByTestId("chat-input"), "/plan design a sliding lid");
    await user.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => expect(client.dispatchAgentStartTurn).toHaveBeenCalled());
    expect(client.dispatchAgentStartTurn).toHaveBeenCalledWith(
      expect.objectContaining({
        mode: "plan",
        prompt: "design a sliding lid",
      }),
    );
    expect(client.dispatchChatSend).toHaveBeenCalledWith(
      expect.objectContaining({
        content: "design a sliding lid",
      }),
    );
  });

  it("renders plan package actions and runs the saved plan", async () => {
    const client = fakeClient();
    const onOpenPlan = vi.fn();
    const onStatus = vi.fn();
    setSnapshot({
      ...chatSnapshot(),
      agent_events: [planSavedEvent()],
    });
    render(
      <ChatZone
        client={client as unknown as WasmClient}
        onOpenPlan={onOpenPlan}
        onStatus={onStatus}
      />,
    );

    expect(screen.getByTestId("plan-package-card")).toBeTruthy();
    expect(screen.getByText("2026050100-add-lid-vents")).toBeTruthy();
    expect(screen.getAllByText("parts/top_lid.py").length).toBeGreaterThan(0);
    expect(screen.getByText("outputs/top_lid.step")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Open Plan" }));
    expect(onOpenPlan).toHaveBeenCalledWith({
      workspace_id: "ws",
      path_segments: ["plans", "2026050100-add-lid-vents", "plan.md"],
    });

    fireEvent.click(screen.getByRole("button", { name: "Run Plan" }));
    await waitFor(() => expect(client.dispatchAgentStartTurn).toHaveBeenCalled());
    expect(client.dispatchAgentStartTurn).toHaveBeenCalledWith(
      expect.objectContaining({
        agent_id: "agent-main",
        mode: "agent",
        plan_ref: {
          workspace_id: "ws",
          path_segments: ["plans", "2026050100-add-lid-vents"],
        },
        prompt: "Run plan 2026050100-add-lid-vents",
        context_refs: [],
      }),
    );
    expect(onStatus).toHaveBeenCalledWith(
      "Running plan 2026050100-add-lid-vents in Agent mode",
    );
  });

  it("writes the first user message when a saved plan creates a draft chat", async () => {
    const client = fakeClient();

    const ok = await runSavedPlan({
      client: client as unknown as WasmClient,
      planId: "2026050100-add-lid-vents",
      planRef: {
        workspace_id: "ws",
        path_segments: ["plans", "2026050100-add-lid-vents"],
      },
      currentSessionId: null,
      sessions: [],
      agentRun: null,
      busy: false,
      contextPills: [],
      agentModelSelection: agentModelSelection(),
      draftClientRequestId: "draft-request",
      setBusy: vi.fn(),
    });

    expect(ok).toBe(true);
    expect(client.dispatchChatCreate).toHaveBeenCalledWith(
      expect.objectContaining({
        client_request_id: "draft-request",
        initial_user_message: "Run plan 2026050100-add-lid-vents",
      }),
    );
    expect(client.dispatchChatSend).not.toHaveBeenCalled();
    expect(client.dispatchAgentInvoke).not.toHaveBeenCalled();
    expect(client.dispatchAgentStartTurn).not.toHaveBeenCalled();
  });

  it("generates a client request id when a saved plan creates chat without a local draft", async () => {
    const client = fakeClient();

    const ok = await runSavedPlan({
      client: client as unknown as WasmClient,
      planId: "2026050100-add-lid-vents",
      planRef: {
        workspace_id: "ws",
        path_segments: ["plans", "2026050100-add-lid-vents"],
      },
      currentSessionId: null,
      sessions: [],
      agentRun: null,
      busy: false,
      contextPills: [],
      agentModelSelection: agentModelSelection(),
      setBusy: vi.fn(),
    });

    expect(ok).toBe(true);
    const createRequest = vi.mocked(client.dispatchChatCreate).mock
      .calls[0][0] as { client_request_id: string };
    expect(createRequest).toEqual(
      expect.objectContaining({
        client_request_id: expect.any(String),
        initial_user_message: "Run plan 2026050100-add-lid-vents",
        initial_turn: expect.objectContaining({
          mode: "agent",
          plan_ref: {
            workspace_id: "ws",
            path_segments: ["plans", "2026050100-add-lid-vents"],
          },
        }),
      }),
    );
    expect(client.dispatchAgentInvoke).not.toHaveBeenCalled();
    expect(client.dispatchChatSend).not.toHaveBeenCalled();
  });

  it("commits saved plan draft when chat create starts the first turn", async () => {
    const client = fakeClient();
    const onStatus = vi.fn();

    const ok = await runSavedPlan({
      client: client as unknown as WasmClient,
      planId: "2026050100-add-lid-vents",
      planRef: {
        workspace_id: "ws",
        path_segments: ["plans", "2026050100-add-lid-vents"],
      },
      currentSessionId: null,
      sessions: [],
      agentRun: null,
      busy: false,
      contextPills: [],
      agentModelSelection: agentModelSelection(),
      onStatus,
      setBusy: vi.fn(),
    });

    expect(ok).toBe(true);
    expect(client.dispatchAgentInvoke).not.toHaveBeenCalled();
    expect(client.dispatchAgentStartTurn).not.toHaveBeenCalled();
    expect(onStatus).not.toHaveBeenCalledWith("invoke failed");
  });

  it("does not render plan package actions when plan_path is outside the package plan", () => {
    const event = planSavedEvent();
    event.payload.package.plan_path = {
      workspace_id: "ws",
      path_segments: ["plans", "2026050100-add-lid-vents", "request.md"],
    };
    setSnapshot({
      ...chatSnapshot(),
      agent_events: [event],
    });
    render(
      <ChatZone
        client={fakeClient() as unknown as WasmClient}
      />,
    );

    expect(screen.queryByTestId("plan-package-card")).toBeNull();
    expect(screen.queryByRole("button", { name: "Open Plan" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Run Plan" })).toBeNull();
  });

  it("renders legacy plan proposals as events without confirmation controls", async () => {
    const client = fakeClient();
    setSnapshot({
      ...chatSnapshot(),
      agent_events: [
        {
          event: "agent.plan_proposed",
          payload: {
            session_id: "main",
            run_id: "run-1",
            plan_ref: { workspace_id: "ws", path_segments: ["plans", "lid.md"] },
            target_path: { workspace_id: "ws", path_segments: ["parts", "lid.py"] },
            target_type: "part",
            affected_files: [{ workspace_id: "ws", path_segments: ["parts", "lid.py"] }],
            export_targets: [{ workspace_id: "ws", path_segments: ["outputs", "lid.step"] }],
            change_description: "increase height",
          },
        },
      ],
    });
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    await waitFor(() => expect(screen.getByText("plan_proposed")).toBeTruthy());
    expect(screen.getByText("increase height")).toBeTruthy();
    await waitFor(() => expect(screen.queryByTestId("plan-confirm-btn")).toBeNull());
    expect(screen.queryByTestId("plan-preview-btn")).toBeNull();
  });

  it("streams only tokens from the current chat session", async () => {
    const client = fakeClient();
    setSnapshot({
      ...chatSnapshot(),
      agent_run: { session_id: "main", run_id: "run-main" },
      agent_events: [
        {
          event: "agent.token",
          payload: { session_id: "other", run_id: "run-other", text: "other text" },
        },
        {
          event: "agent.token",
          payload: { session_id: "main", run_id: "run-main", text: "main text" },
        },
      ],
    });
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    await waitFor(() => expect(screen.getByText("main text")).toBeTruthy());
    expect(screen.queryByText("other text")).toBeNull();
  });

  it("streams only tokens from the current chat agent", async () => {
    const client = fakeClient();
    setSnapshot({
      ...chatSnapshot(),
      agent_run: { session_id: "main", agent_id: "agent-main", run_id: "run-main" },
      agent_events: [
        {
          event: "agent.token",
          payload: { agent_id: "agent-other", run_id: "run-other", text: "other agent text" },
        },
        {
          event: "agent.token",
          payload: { agent_id: "agent-main", run_id: "run-main", text: "main agent text" },
        },
      ],
    });
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    await waitFor(() => expect(screen.getByText("main agent text")).toBeTruthy());
    expect(screen.queryByText("other agent text")).toBeNull();
  });

  it("renders live agent tokens and tool events in arrival order", async () => {
    const client = fakeClient();
    setSnapshot({
      ...chatSnapshot(),
      agent_run: { session_id: "main", run_id: "run-main" },
      agent_events: [
        {
          event: "agent.token",
          payload: { session_id: "main", run_id: "run-main", text: "First answer." },
        },
        {
          event: "agent.tool_start",
          payload: {
            session_id: "main",
            run_id: "run-main",
            tool_name: "read_file",
          },
        },
        {
          event: "agent.tool_result",
          payload: {
            session_id: "main",
            run_id: "run-main",
            tool_name: "read_file",
            result_json: "{\"ok\":true}",
          },
        },
        {
          event: "agent.token",
          payload: { session_id: "main", run_id: "run-main", text: "Second answer." },
        },
      ],
    });
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    const first = await screen.findByText(/First answer/);
    const tool = screen.getByText("read_file");
    const second = screen.getByText(/Second answer/);
    expect(isBefore(first, tool)).toBe(true);
    expect(isBefore(tool, second)).toBe(true);
  });

  it("keeps streamed text visible after done until history covers it", async () => {
    const client = fakeClient();
    setSnapshot({
      ...chatSnapshot(),
      agent_run: null,
      agent_events: [
        {
          event: "agent.token",
          payload: { session_id: "main", run_id: "run-main", text: "Final answer." },
        },
        {
          event: "agent.done",
          payload: { session_id: "main", run_id: "run-main", cancelled: false },
        },
      ],
    });
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    await waitFor(() => expect(screen.getByText(/Final answer/)).toBeTruthy());
  });

  it("keeps every live token chunk for long running answers", async () => {
    const client = fakeClient();
    setSnapshot({
      ...chatSnapshot(),
      agent_run: { session_id: "main", run_id: "run-main" },
      agent_events: Array.from({ length: 90 }, (_, index) => ({
        event: "agent.token",
        payload: {
          session_id: "main",
          run_id: "run-main",
          text: `chunk-${index} `,
        },
      })),
    });
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    await waitFor(() => expect(screen.getByText(/chunk-0/)).toBeTruthy());
    expect(screen.getByText(/chunk-89/)).toBeTruthy();
  });

  it("does not hide current live tokens when old assistant text matches", async () => {
    const client = fakeClient();
    setSnapshot({
      ...chatSnapshot(),
      current_chat_history: [
        chatMessage("old-answer", "assistant", "Done."),
      ],
      agent_run: { session_id: "main", run_id: "run-main" },
      agent_events: [
        {
          event: "agent.token",
          payload: { session_id: "main", run_id: "run-main", text: "Done." },
        },
      ],
    });
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    await waitFor(() => expect(screen.getAllByText(/Done\./).length).toBeGreaterThanOrEqual(2));
  });

  it("keeps done text visible when old assistant text matches before refresh", async () => {
    const client = fakeClient();
    setSnapshot({
      ...chatSnapshot(),
      current_chat_history: [
        chatMessage("old-question", "user", "previous request"),
        chatMessage("old-answer", "assistant", "Done."),
      ],
      agent_run: null,
      agent_events: [
        {
          event: "agent.token",
          payload: { session_id: "main", run_id: "run-main", text: "Done." },
        },
        {
          event: "agent.done",
          payload: { session_id: "main", run_id: "run-main", cancelled: false },
        },
      ],
    });
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    await waitFor(() => expect(screen.getAllByText(/Done\./).length).toBeGreaterThanOrEqual(2));
  });

  it("shows answer exactly once after history refresh by preferring events", async () => {
    const client = fakeClient();
    setSnapshot({
      ...chatSnapshot(),
      current_chat_history: [
        chatMessage("current-question", "user", "current request"),
      ],
      agent_run: { session_id: "main", run_id: "run-main" },
      agent_events: [
        {
          event: "agent.token",
          payload: {
            session_id: "main",
            run_id: "run-main",
            text: "Fresh answer.",
          },
        },
      ],
    });
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    await waitFor(() => expect(screen.getByText(/Fresh answer/)).toBeTruthy());

    setSnapshot({
      ...chatSnapshot(),
      current_chat_history: [
        chatMessage("current-question", "user", "current request"),
        chatMessage("current-answer", "assistant", "Fresh answer.", "run-main"),
      ],
      agent_run: null,
      agent_events: [
        {
          event: "agent.token",
          payload: {
            session_id: "main",
            run_id: "run-main",
            text: "Fresh answer.",
          },
        },
        {
          event: "agent.done",
          payload: { session_id: "main", run_id: "run-main", cancelled: false },
        },
      ],
    });

    await waitFor(() => expect(screen.getAllByText(/Fresh answer\./)).toHaveLength(1));
  });

  it("keeps the thinking indicator visible while a tool call is in progress", async () => {
    const client = fakeClient();
    setSnapshot({
      ...chatSnapshot(),
      agent_run: { session_id: "main", run_id: "run-main" },
      agent_events: [
        {
          event: "agent.token",
          payload: { session_id: "main", run_id: "run-main", text: "Checking files." },
        },
        {
          event: "agent.tool_start",
          payload: {
            session_id: "main",
            run_id: "run-main",
            tool_name: "read_file",
          },
        },
      ],
    });
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    await waitFor(() => expect(screen.getByTestId("agent-thinking")).toBeTruthy());
    expect(screen.getByTestId("agent-thinking").textContent).toContain("Thinking");
  });

  it("renders the latest live reasoning process with a Thinking label", async () => {
    const client = fakeClient();
    setSnapshot({
      ...chatSnapshot(),
      agent_run: { session_id: "main", run_id: "run-main" },
      agent_events: [
        {
          event: "agent.reasoning",
          payload: {
            session_id: "main",
            run_id: "run-main",
            text: "First reasoning step.",
          },
        },
        {
          event: "agent.tool_start",
          payload: {
            session_id: "main",
            run_id: "run-main",
            tool_name: "cadquery_execute",
          },
        },
        {
          event: "agent.reasoning",
          payload: {
            session_id: "main",
            run_id: "run-main",
            text: "Latest reasoning step.",
          },
        },
      ],
    });
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    await waitFor(() => expect(screen.getByTestId("agent-reasoning")).toBeTruthy());
    expect(screen.getByTestId("agent-reasoning").textContent).toContain("Thinking");
    expect(screen.getByText("Latest reasoning step.")).toBeTruthy();
    expect(screen.queryByText("First reasoning step.")).toBeNull();
  });

  it("scrolls chat body to the newest live content", async () => {
    setSnapshot({
      ...chatSnapshot(),
      current_chat_history: [
        chatMessage("msg-1", "user", "make a box"),
        chatMessage("msg-2", "assistant", "done"),
      ],
    });
    render(
      <ChatZone
        client={fakeClient() as unknown as WasmClient}
      />,
    );

    await waitFor(() => expect(screen.getByText("make a box")).toBeTruthy());
    expect(screen.getByText("done")).toBeTruthy();
    expect(screen.getByTestId("chat-body")).toBeTruthy();
  });

  it("resets streaming text when switching chat sessions", async () => {
    const client = fakeClient();
    setSnapshot({
      ...chatSnapshot(),
      agent_run: { session_id: "main", run_id: "run-main" },
      agent_events: [
        {
          event: "agent.token",
          payload: { session_id: "main", run_id: "run-main", text: "main text" },
        },
      ],
    });
    render(
      <ChatZone
        client={client as unknown as WasmClient}
      />,
    );

    await waitFor(() => expect(screen.getByText("main text")).toBeTruthy());

    setSnapshot({
      ...chatSnapshot(),
      chat_sessions: [
        { session_id: "main", title: "main", archived: false, message_count: 1 },
        { session_id: "other", title: "other", archived: false, message_count: 1 },
      ],
      current_chat_session: "other",
      agent_run: { session_id: "other", run_id: "run-other" },
      agent_events: [
        {
          event: "agent.token",
          payload: { session_id: "other", run_id: "run-other", text: "other text" },
        },
      ],
    });

    await waitFor(() => expect(screen.getByText("other text")).toBeTruthy());
    expect(screen.queryByText("main text")).toBeNull();
  });

  it("shows welcome empty state when no messages or events", () => {
    setSnapshot(chatSnapshot());
    render(
      <ChatZone
        client={null}
      />,
    );
    expect(screen.getByTestId("chat-empty-state")).toBeTruthy();
  });

  it("shows LLM setup guide when llm_configured is false", () => {
    setSnapshot({ ...chatSnapshot(), llm_configured: false });
    render(
      <ChatZone
        client={null}
      />,
    );
    expect(screen.getByTestId("llm-setup-guide")).toBeTruthy();
  });
});

function isBefore(left: Element, right: Element): boolean {
  return Boolean(left.compareDocumentPosition(right) & Node.DOCUMENT_POSITION_FOLLOWING);
}

function chatSnapshot(
  currentSelection: ChatSnapshot["current_selection"] = null,
  agentModelRegistry: ChatSnapshot["agent_model_registry"] = null,
): ChatSnapshot {
  return {
    workspace_current: { workspace_id: "ws" },
    chat_sessions: [
      {
        session_id: "main",
        agent_id: "agent-main",
        title: "main",
        archived: false,
        message_count: 1,
      },
    ],
    current_chat_session: "main",
    current_chat_history: [],
    agent_run: null,
    agent_events: [],
    current_selection: currentSelection,
    agent_model_registry: agentModelRegistry,
  };
}

function agentModelRegistry(): NonNullable<ChatSnapshot["agent_model_registry"]> {
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
      {
        id: "anthropic",
        kind: "anthropic",
        label: "Anthropic",
        discovery: {
          enabled: true,
          status: "failed",
          error: "manual fallback remains available",
        },
        models: [
          {
            id: "claude-sonnet",
            label: "Claude Sonnet",
            source: "manual",
            reasoning_effort: "medium",
            service_label: null,
            native_web_search_enabled: true,
            native_web_search_applied: false,
            web_search_supported: false,
            web_search_unsupported_reason: "model does not support web search",
            search_sources_supported: false,
          },
        ],
      },
    ],
  };
}

function agentModelSelection() {
  return {
    provider_id: "openai",
    provider_type: "openai_responses" as const,
    model_id: "gpt-5.2",
    reasoning_effort: "high",
    service_label: "flex",
  };
}

function faceSelection() {
  return {
    kind: "face" as const,
    ref_text: "@face[top_lid:f_0]",
    owner_ref_text: "@part[top_lid]",
    owner_object_kind: "part" as const,
    instance_path: null,
    candidate_feature_ref: "@feature[top_lid.lid_alignment_surface]",
    build_id: "sha256:build",
    result_id: "cq_1",
    ambiguous: false,
  };
}

function planSavedEvent() {
  return {
    event: "agent.plan_saved",
    payload: {
      session_id: "main",
      run_id: "run-1",
      package: {
        plan_id: "2026050100-add-lid-vents",
        plan_ref: {
          workspace_id: "ws",
          path_segments: ["plans", "2026050100-add-lid-vents"],
        },
        request_path: {
          workspace_id: "ws",
          path_segments: ["plans", "2026050100-add-lid-vents", "request.md"],
        },
        plan_path: {
          workspace_id: "ws",
          path_segments: ["plans", "2026050100-add-lid-vents", "plan.md"],
        },
        result_path: {
          workspace_id: "ws",
          path_segments: ["plans", "2026050100-add-lid-vents", "plan-result.md"],
        },
      },
      title: "Add lid vents",
      status: "planned",
      target_path: { workspace_id: "ws", path_segments: ["parts", "top_lid.py"] },
      target_type: "part",
      affected_files: [
        { workspace_id: "ws", path_segments: ["parts", "top_lid.py"] },
      ],
      new_files: [],
      change_description: "Add cooling vents to the lid",
      export_targets: [
        { workspace_id: "ws", path_segments: ["outputs", "top_lid.step"] },
      ],
    },
  };
}

function chatMessage(
  message_id: string,
  role: "user" | "assistant" | "tool" | "meta",
  content: string,
  run_id?: string | null,
) {
  return {
    message_id,
    ts_ms: 1,
    role,
    content,
    tool_calls: [],
    tool_result: null,
    mesh_result: null,
    run_id: run_id ?? null,
  };
}

function setNativeTextareaValue(
  input: HTMLTextAreaElement,
  value: string,
): void {
  const setter = Object.getOwnPropertyDescriptor(
    HTMLTextAreaElement.prototype,
    "value",
  )?.set;
  setter?.call(input, value);
}

function fakeClient(): Pick<
  WasmClient,
  | "dispatchChatList"
  | "dispatchChatCreate"
  | "dispatchChatSend"
  | "dispatchAgentInvoke"
  | "dispatchAgentStartTurn"
  | "dispatchAgentSnapshot"
  | "dispatchAgentSubscribe"
  | "dispatchChatHistory"
  | "dispatchAgentCancel"
  | "dispatchAgentModelRegistry"
  | "dispatchAgentModelSelect"
  | "dispatchAgentModelParamsUpdate"
  | "dispatchCadQueryPreview"
> {
  return {
    dispatchChatList: vi.fn().mockResolvedValue({}),
    dispatchChatCreate: vi.fn().mockResolvedValue({ session_id: "main" }),
    dispatchChatSend: vi.fn().mockResolvedValue({}),
    dispatchAgentInvoke: vi.fn().mockResolvedValue({}),
    dispatchAgentStartTurn: vi.fn().mockResolvedValue({}),
    dispatchAgentSnapshot: vi.fn().mockResolvedValue({ events: [] }),
    dispatchAgentSubscribe: vi.fn().mockResolvedValue({}),
    dispatchChatHistory: vi.fn().mockResolvedValue({}),
    dispatchAgentCancel: vi.fn().mockResolvedValue({}),
    dispatchAgentModelRegistry: vi.fn().mockResolvedValue({}),
    dispatchAgentModelSelect: vi.fn().mockResolvedValue({}),
    dispatchAgentModelParamsUpdate: vi.fn().mockResolvedValue({}),
    dispatchCadQueryPreview: vi.fn().mockResolvedValue({}),
  };
}

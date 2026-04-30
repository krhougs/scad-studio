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
import { ChatZone, type ChatSnapshot } from "../../src/workbench/chat-zone";
import type { WasmClient } from "../../src/wasm-bridge";

const STORE_DEFAULTS = {
  chat_sessions: [] as ChatSnapshot["chat_sessions"],
  current_chat_session: null,
  current_chat_history: [] as ChatSnapshot["current_chat_history"],
  agent_run: null,
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

    await waitFor(() => expect(client.dispatchAgentInvoke).toHaveBeenCalled());
    expect(client.dispatchAgentInvoke).toHaveBeenCalledWith(
      expect.objectContaining({
        mode: "agent",
        plan_ref: null,
        context_refs: [],
      }),
    );
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

    await waitFor(() => expect(client.dispatchAgentInvoke).toHaveBeenCalled());
    expect(client.dispatchAgentInvoke).toHaveBeenCalledWith(
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

    await waitFor(() => expect(client.dispatchAgentInvoke).toHaveBeenCalled());
    expect(client.dispatchAgentInvoke).toHaveBeenCalledWith(
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

    await waitFor(() => expect(client.dispatchAgentInvoke).toHaveBeenCalled());
    expect(client.dispatchAgentInvoke).toHaveBeenCalledWith(
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

    await waitFor(() => expect(client.dispatchAgentInvoke).toHaveBeenCalled());
    expect(client.dispatchAgentInvoke).toHaveBeenCalledWith(
      expect.objectContaining({
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

    await waitFor(() => expect(client.dispatchAgentInvoke).toHaveBeenCalled());
    expect(client.dispatchAgentInvoke).toHaveBeenCalledWith(
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
    await waitFor(() => expect(client.dispatchAgentInvoke).toHaveBeenCalled());
    expect(client.dispatchAgentInvoke).toHaveBeenCalledWith(
      expect.objectContaining({
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
): ChatSnapshot {
  return {
    workspace_current: { workspace_id: "ws" },
    chat_sessions: [
      {
        session_id: "main",
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
  | "dispatchChatHistory"
  | "dispatchAgentCancel"
  | "dispatchCadQueryPreview"
> {
  return {
    dispatchChatList: vi.fn().mockResolvedValue({}),
    dispatchChatCreate: vi.fn().mockResolvedValue({ session_id: "main" }),
    dispatchChatSend: vi.fn().mockResolvedValue({}),
    dispatchAgentInvoke: vi.fn().mockResolvedValue({}),
    dispatchChatHistory: vi.fn().mockResolvedValue({}),
    dispatchAgentCancel: vi.fn().mockResolvedValue({}),
    dispatchCadQueryPreview: vi.fn().mockResolvedValue({}),
  };
}

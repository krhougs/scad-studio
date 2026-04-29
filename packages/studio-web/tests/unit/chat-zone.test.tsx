import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ChatZone, type ChatSnapshot } from "../../src/workbench/chat-zone";
import type { WasmClient } from "../../src/wasm-bridge";

describe("ChatZone", () => {
  afterEach(cleanup);

  it("sends agent invoke without plan_ref", async () => {
    const client = fakeClient();
    render(
      <ChatZone
        client={client as unknown as WasmClient}
        snapshot={chatSnapshot()}
      />,
    );

    fireEvent.change(screen.getByTestId("chat-input"), {
      target: { value: "make the lid taller" },
    });
    fireEvent.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => expect(client.dispatchAgentInvoke).toHaveBeenCalled());
    expect(client.dispatchAgentInvoke).toHaveBeenCalledWith(
      expect.objectContaining({
        mode: "agent",
        plan_ref: null,
        context_refs: [],
      }),
    );
  });

  it("sends the mode selected in the composer dropdown", async () => {
    const client = fakeClient();
    render(
      <ChatZone
        client={client as unknown as WasmClient}
        snapshot={chatSnapshot()}
      />,
    );

    const modeSelect = screen.getByLabelText("agent mode");
    expect((modeSelect as HTMLSelectElement).value).toBe("agent");

    fireEvent.change(modeSelect, { target: { value: "plan" } });
    fireEvent.change(screen.getByTestId("chat-input"), {
      target: { value: "apply the latest plan" },
    });
    fireEvent.click(screen.getByRole("button", { name: /send/i }));

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
    render(
      <ChatZone
        client={client as unknown as WasmClient}
        snapshot={chatSnapshot()}
      />,
    );

    fireEvent.change(screen.getByLabelText("agent mode"), {
      target: { value: "plan" },
    });
    fireEvent.change(screen.getByTestId("chat-input"), {
      target: { value: "/agent explain CadQuery loft" },
    });
    fireEvent.click(screen.getByRole("button", { name: /send/i }));

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
    render(
      <ChatZone
        client={client as unknown as WasmClient}
        snapshot={{
          ...chatSnapshot(),
          agent_events: [
            {
              event: "agent.done",
              payload: { run_id: "run-1", cancelled: false },
            },
          ],
        }}
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
    render(
      <ChatZone
        client={client as unknown as WasmClient}
        snapshot={{
          ...chatSnapshot(),
          current_chat_session: null,
          current_chat_history: [],
        }}
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
    render(
      <ChatZone
        client={client as unknown as WasmClient}
        snapshot={chatSnapshot({
          selections: [faceSelection()],
          active_index: 0,
        })}
      />,
    );

    expect(screen.getByTestId("context-pill-bar")).toBeTruthy();
    expect(screen.getByTestId("context-pill-bar").textContent).toContain(
      "@feature[top_lid.top_surface]",
    );

    fireEvent.change(screen.getByTestId("chat-input"), {
      target: { value: "open a slot on this face" },
    });
    fireEvent.click(screen.getByRole("button", { name: /send/i }));

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
    render(
      <ChatZone
        client={client as unknown as WasmClient}
        snapshot={chatSnapshot()}
      />,
    );

    fireEvent.change(screen.getByTestId("chat-input"), {
      target: { value: "/plan design a sliding lid" },
    });
    fireEvent.click(screen.getByRole("button", { name: /send/i }));

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
    render(
      <ChatZone
        client={client as unknown as WasmClient}
        snapshot={{
          ...chatSnapshot(),
          agent_events: [planSavedEvent()],
        }}
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
    render(
      <ChatZone
        client={fakeClient() as unknown as WasmClient}
        snapshot={{
          ...chatSnapshot(),
          agent_events: [event],
        }}
      />,
    );

    expect(screen.queryByTestId("plan-package-card")).toBeNull();
    expect(screen.queryByRole("button", { name: "Open Plan" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Run Plan" })).toBeNull();
  });

  it("renders legacy plan proposals as events without confirmation controls", async () => {
    const client = fakeClient();
    render(
      <ChatZone
        client={client as unknown as WasmClient}
        snapshot={{
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
        }}
      />,
    );

    await waitFor(() => expect(screen.getByText("plan_proposed")).toBeTruthy());
    expect(screen.getByText("increase height")).toBeTruthy();
    await waitFor(() => expect(screen.queryByTestId("plan-confirm-btn")).toBeNull());
    expect(screen.queryByTestId("plan-preview-btn")).toBeNull();
  });

  it("streams only tokens from the current chat session", async () => {
    const client = fakeClient();
    render(
      <ChatZone
        client={client as unknown as WasmClient}
        snapshot={{
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
        }}
      />,
    );

    await waitFor(() => expect(screen.getByText("main text")).toBeTruthy());
    expect(screen.queryByText("other text")).toBeNull();
  });

  it("resets streaming text when switching chat sessions", async () => {
    const client = fakeClient();
    const { rerender } = render(
      <ChatZone
        client={client as unknown as WasmClient}
        snapshot={{
          ...chatSnapshot(),
          agent_run: { session_id: "main", run_id: "run-main" },
          agent_events: [
            {
              event: "agent.token",
              payload: { session_id: "main", run_id: "run-main", text: "main text" },
            },
          ],
        }}
      />,
    );

    await waitFor(() => expect(screen.getByText("main text")).toBeTruthy());

    rerender(
      <ChatZone
        client={client as unknown as WasmClient}
        snapshot={{
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
        }}
      />,
    );

    await waitFor(() => expect(screen.getByText("other text")).toBeTruthy());
    expect(screen.queryByText("main text")).toBeNull();
  });

  it("shows welcome empty state when no messages or events", () => {
    render(
      <ChatZone
        client={null}
        snapshot={chatSnapshot()}
      />,
    );
    expect(screen.getByTestId("chat-empty-state")).toBeTruthy();
  });
});

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
    candidate_feature_ref: "@feature[top_lid.top_surface]",
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

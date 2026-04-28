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

  it("sends auto invoke without confirmed_cadquery", async () => {
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
        operation: "auto",
        confirmed_cadquery: null,
        context_refs: [],
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
        operation: "auto",
        confirmed_cadquery: null,
        context_refs: ["@face[top_lid:f_0]"],
      }),
    );
  });

  it("sends explicit operation when using slash command", async () => {
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
        operation: "plan",
        prompt: "design a sliding lid",
      }),
    );
    expect(client.dispatchChatSend).toHaveBeenCalledWith(
      expect.objectContaining({
        content: "design a sliding lid",
      }),
    );
  });

  it("dispatches cadquery preview when clicking plan preview button", async () => {
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

    await waitFor(() => expect(screen.getByTestId("plan-preview-btn")).toBeTruthy());
    fireEvent.click(screen.getByTestId("plan-preview-btn"));

    await waitFor(() => expect(client.dispatchCadQueryPreview).toHaveBeenCalled());
    expect(client.dispatchCadQueryPreview).toHaveBeenCalledWith(
      expect.objectContaining({
        target_path: { workspace_id: "ws", path_segments: ["parts", "lid.py"] },
        export_formats: [],
      }),
    );
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

function fakeClient(): Pick<
  WasmClient,
  | "dispatchChatList"
  | "dispatchChatCreate"
  | "dispatchChatSend"
  | "dispatchAgentInvoke"
  | "dispatchChatHistory"
  | "dispatchAgentCancel"
  | "dispatchAgentPlanConfirm"
  | "dispatchAgentPlanReject"
  | "dispatchCadQueryPreview"
> {
  return {
    dispatchChatList: vi.fn().mockResolvedValue({}),
    dispatchChatCreate: vi.fn().mockResolvedValue({ session_id: "main" }),
    dispatchChatSend: vi.fn().mockResolvedValue({}),
    dispatchAgentInvoke: vi.fn().mockResolvedValue({}),
    dispatchChatHistory: vi.fn().mockResolvedValue({}),
    dispatchAgentCancel: vi.fn().mockResolvedValue({}),
    dispatchAgentPlanConfirm: vi.fn().mockResolvedValue({}),
    dispatchAgentPlanReject: vi.fn().mockResolvedValue({}),
    dispatchCadQueryPreview: vi.fn().mockResolvedValue({}),
  };
}

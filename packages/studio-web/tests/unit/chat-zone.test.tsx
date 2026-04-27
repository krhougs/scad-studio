import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ChatZone, type ChatSnapshot } from "../../src/workbench/chat-zone";
import type { WasmClient } from "../../src/wasm-bridge";

describe("ChatZone", () => {
  it("sends execute invokes with structured cadquery confirmation", async () => {
    const client = fakeClient();
    render(
      <ChatZone
        client={client as unknown as WasmClient}
        snapshot={chatSnapshot()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Execute" }));
    fireEvent.change(screen.getByTestId("chat-input"), {
      target: { value: "make the lid taller" },
    });
    fireEvent.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => expect(client.dispatchAgentInvoke).toHaveBeenCalled());
    expect(client.dispatchAgentInvoke).toHaveBeenCalledWith(
      expect.objectContaining({
        operation: "execute",
        confirmed_cadquery: expect.objectContaining({
          request: expect.objectContaining({
            target_path: {
              workspace_id: "ws",
              path_segments: ["parts", "agent_model.py"],
            },
            code: "",
          }),
          affected_files: [
            {
              workspace_id: "ws",
              path_segments: ["parts", "agent_model.py"],
            },
          ],
        }),
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
});

function chatSnapshot(): ChatSnapshot {
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
> {
  return {
    dispatchChatList: vi.fn().mockResolvedValue({}),
    dispatchChatCreate: vi.fn().mockResolvedValue({ session_id: "main" }),
    dispatchChatSend: vi.fn().mockResolvedValue({}),
    dispatchAgentInvoke: vi.fn().mockResolvedValue({}),
    dispatchChatHistory: vi.fn().mockResolvedValue({}),
    dispatchAgentCancel: vi.fn().mockResolvedValue({}),
  };
}

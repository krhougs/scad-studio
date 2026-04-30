import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { AgentEventRow } from "../../src/workbench/chat-messages";

describe("AgentEventRow", () => {
  afterEach(() => cleanup());

  it("renders done as a compact assistant mark", () => {
    render(
      <AgentEventRow
        event={{ event: "agent.done", payload: { cancelled: false } }}
      />,
    );

    expect(screen.getByTestId("agent-done-mark").textContent).toContain(
      "budn'",
    );
    expect(screen.queryByTestId("agent-event-row")).toBeNull();
  });

  it("collapses tool details into a single line with modal detail", () => {
    render(
      <AgentEventRow
        event={{
          event: "agent.tool_result",
          payload: {
            tool_name: "cadquery_execute",
            result_json: "{\"status\":\"ok\",\"result_id\":\"cq_123\"}",
          },
        }}
      />,
    );

    expect(screen.getByTestId("agent-event-row").textContent).toContain(
      "cadquery_execute",
    );
    expect(screen.queryByTestId("agent-event-modal")).toBeNull();

    fireEvent.click(screen.getByTestId("agent-event-row"));
    expect(screen.getByTestId("agent-event-modal").textContent).toContain(
      "\"result_id\":\"cq_123\"",
    );
  });
});

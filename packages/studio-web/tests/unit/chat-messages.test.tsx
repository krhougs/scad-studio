import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it } from "vitest";
import { AgentEventRow, SearchSourcesPart } from "../../src/workbench/chat-messages";

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
            tool_call_id: "call_123",
            run_id: "run_456",
            result_json: "{\"status\":\"ok\",\"result_id\":\"cq_123\"}",
          },
        }}
      />,
    );

    expect(screen.getByTestId("agent-event-row").textContent).toContain(
      "cadquery_execute",
    );
    expect(screen.getByTestId("agent-event-row").textContent).toContain(
      "result ready",
    );
    expect(screen.queryByTestId("agent-event-modal")).toBeNull();

    fireEvent.click(screen.getByTestId("agent-event-row"));
    expect(screen.getByTestId("agent-event-modal").textContent).toContain(
      "cq_123",
    );
    expect(screen.getByTestId("agent-event-modal").textContent).toContain(
      "call_123",
    );
    expect(screen.getByTestId("agent-event-modal").textContent).toContain(
      "run_456",
    );
  });

  it("keeps tool start metadata available in modal detail", () => {
    render(
      <AgentEventRow
        event={{
          event: "agent.tool_start",
          payload: {
            tool_name: "cadquery_execute",
            tool_call_id: "call_789",
            run_id: "run_999",
            args_json: "{\"path\":\"parts/fixture_panel.py\"}",
          },
        }}
      />,
    );

    expect(screen.getByTestId("agent-event-row").textContent).toContain(
      "cadquery_execute",
    );
    expect(screen.queryByTestId("agent-event-modal")).toBeNull();

    fireEvent.click(screen.getByTestId("agent-event-row"));
    const modalText = screen.getByTestId("agent-event-modal").textContent ?? "";
    expect(modalText).toContain("parts/fixture_panel.py");
    expect(modalText).toContain("call_789");
    expect(modalText).toContain("run_999");
  });
});

describe("SearchSourcesPart", () => {
  afterEach(() => cleanup());

  it("renders http search sources and filters unsafe urls", () => {
    render(
      <SearchSourcesPart
        name="agent.search_sources"
        data={{
          sources: [
            {
              title: "OpenAI web search",
              url: "https://developers.openai.com/api/docs/guides/tools-web-search",
            },
            { title: "unsafe", url: "javascript:alert(1)" },
          ],
        }}
      />,
    );

    const sources = screen.getByTestId("agent-search-sources");
    expect(sources.textContent).toContain("OpenAI web search");
    expect(sources.textContent).not.toContain("unsafe");
    const link = screen.getByRole("link", { name: "OpenAI web search" });
    expect(link.getAttribute("href")).toBe(
      "https://developers.openai.com/api/docs/guides/tools-web-search",
    );
    expect(link.getAttribute("rel")).toBe("noreferrer");
  });
});

describe("chat source styles", () => {
  afterEach(() => cleanup());

  it("hides only consecutive assistant sources", () => {
    const style = document.createElement("style");
    style.textContent = readFileSync(
      "src/styles/workbench-zones.css",
      "utf8",
    );
    document.head.appendChild(style);
    document.body.innerHTML = `
      <div class="msg user"><div class="who"><b>user</b></div></div>
      <div class="msg agent"><div class="who"><b>assistant</b></div></div>
      <div class="msg agent"><div class="who"><b>assistant</b></div></div>
      <div class="msg user"><div class="who"><b>user</b></div></div>
    `;

    const sources = Array.from(document.querySelectorAll<HTMLElement>(".who"));
    expect(getComputedStyle(sources[0]!).display).not.toBe("none");
    expect(getComputedStyle(sources[1]!).display).not.toBe("none");
    expect(getComputedStyle(sources[2]!).display).toBe("none");
    expect(getComputedStyle(sources[3]!).display).not.toBe("none");
  });
});

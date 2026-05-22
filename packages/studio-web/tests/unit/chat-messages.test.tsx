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

  it("renders cadquery tool result as a CadQueryToolCard", () => {
    render(
      <AgentEventRow
        event={{
          event: "agent.tool_result",
          payload: {
            tool_name: "cadquery_execute",
            tool_call_id: "call_123",
            run_id: "run_456",
            result_json: "{\"status\":\"ok\",\"result_id\":\"cq_123\",\"committed_files\":[\"parts/lid.py\"],\"exports\":[\"parts/lid.step\"]}",
          },
        }}
      />,
    );

    const card = screen.getByTestId("cadquery-tool-card");
    expect(card.textContent).toContain("execute");
    expect(card.textContent).toContain("success");
    expect(card.textContent).toContain("parts/lid.py");
    expect(card.textContent).toContain("parts/lid.step");
  });

  it("renders cadquery tool start as running card with target path", () => {
    render(
      <AgentEventRow
        event={{
          event: "agent.tool_start",
          payload: {
            tool_name: "cadquery_execute",
            tool_call_id: "call_789",
            run_id: "run_999",
            args_json: "{\"target_path\":\"parts/fixture_panel.py\"}",
          },
        }}
      />,
    );

    const card = screen.getByTestId("cadquery-tool-card");
    expect(card.textContent).toContain("execute");
    expect(card.textContent).toContain("running");
    expect(card.textContent).toContain("parts/fixture_panel.py");
  });

  it("renders cadquery error with collapsible diagnostics", () => {
    render(
      <AgentEventRow
        event={{
          event: "agent.tool_result",
          payload: {
            tool_name: "cadquery_dry_run",
            tool_call_id: "call_err",
            run_id: "run_err",
            result_json: JSON.stringify({
              status: "error",
              error_type: "geometry_invalid",
              message: "Empty shape after fillet",
              diagnostics: { traceback: "line 1\nline 2\nline 3\nline 4" },
            }),
          },
        }}
      />,
    );

    const card = screen.getByTestId("cadquery-tool-card");
    expect(card.textContent).toContain("dry_run");
    expect(card.textContent).toContain("geometry_invalid");
    expect(card.textContent).toContain("Empty shape after fillet");
    expect(card.querySelector(".cadquery-tool-traceback")).toBeNull();

    fireEvent.click(screen.getByText("show diagnostics"));
    const pre = card.querySelector(".cadquery-tool-traceback");
    expect(pre).not.toBeNull();
    expect(pre!.textContent).toContain("line 1");
    expect(pre!.textContent).toContain("line 3");
    expect(pre!.textContent).not.toContain("line 4");
  });

  it("renders non-cadquery tool events as generic rows", () => {
    render(
      <AgentEventRow
        event={{
          event: "agent.tool_result",
          payload: {
            tool_name: "read_file",
            tool_call_id: "call_rf",
            run_id: "run_rf",
            result_json: "{\"status\":\"ok\"}",
          },
        }}
      />,
    );

    expect(screen.queryByTestId("cadquery-tool-card")).toBeNull();
    expect(screen.getByTestId("agent-event-row")).toBeTruthy();
  });

  it("renders hosted web search requested as searched", () => {
    render(
      <AgentEventRow
        event={{
          event: "agent.hosted_tool_activity",
          payload: {
            tool_type: "web_search",
            status: "requested",
            provider_id: "openai",
          },
        }}
      />,
    );

    expect(screen.getByTestId("agent-event-row").textContent).toContain(
      "searched",
    );
    expect(screen.getByTestId("agent-event-row").textContent).not.toContain(
      "web_search",
    );
  });

  it("does not add product copy for other hosted tools", () => {
    render(
      <AgentEventRow
        event={{
          event: "agent.hosted_tool_activity",
          payload: {
            tool_type: "code_interpreter",
            status: "requested",
          },
        }}
      />,
    );

    expect(screen.queryByText("searched")).toBeNull();
    expect(screen.getByTestId("agent-event-row").textContent).toContain(
      "hosted_tool_activity",
    );
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
